use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct RuntimeConfig {
    pub agent: AgentConfig,
    pub scraper: ScraperConfig,
    pub storage: StorageConfig,
    pub ctrip: CtripConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub notification: NotificationConfig,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub max_turns: u32,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
}

impl AgentConfig {
    /// Resolve API key: config file > env var ANTHROPIC_API_KEY.
    pub fn resolve_api_key(&self) -> Option<String> {
        if !self.api_key.is_empty() {
            return Some(self.api_key.clone());
        }
        std::env::var("ANTHROPIC_API_KEY").ok()
    }

    /// Resolve base URL: config file > env var ANTHROPIC_BASE_URL > official default.
    pub fn resolve_base_url(&self) -> String {
        if !self.base_url.is_empty() {
            return self.base_url.clone();
        }
        std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct ScraperConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub db_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CtripConfig {
    pub default_city: String,
    pub default_adults: u8,
    pub default_children: u8,
    pub request_delay_ms: u64,
    pub max_concurrent: u8,
    pub proxy_pool: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default = "default_redis_ttl")]
    pub ttl_seconds: u64,
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_redis_ttl() -> u64 {
    3600
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: default_redis_url(),
            ttl_seconds: default_redis_ttl(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook_urls: Vec<String>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_urls: Vec::new(),
        }
    }
}

impl RuntimeConfig {
    /// Load config from a single explicit path (legacy behavior).
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self> {
        let candidates = ["config.toml", "cctraveler.toml"];
        for name in &candidates {
            let path = Path::new(name);
            if path.exists() {
                return Self::load(path);
            }
        }
        anyhow::bail!("No config file found (tried: config.toml, cctraveler.toml)")
    }

    /// Layered config loader. Search order (later wins, missing files OK):
    ///   1. ~/.cctraveler/config.toml          (user)
    ///   2. ./.cctraveler/config.toml          (project)
    ///   3. ./config.toml or ./cctraveler.toml (legacy / repo dev)
    ///   4. explicit_path (if provided)
    ///
    /// At least one source must yield a config; otherwise returns error
    /// with a helpful message pointing at `cctraveler init`.
    pub fn load_layered(explicit_path: Option<&Path>) -> Result<Self> {
        let mut sources: Vec<PathBuf> = Vec::new();

        if let Some(home) = user_config_path() {
            if home.exists() {
                sources.push(home);
            }
        }
        let project = PathBuf::from(".cctraveler/config.toml");
        if project.exists() {
            sources.push(project);
        }
        for legacy in ["config.toml", "cctraveler.toml"] {
            let p = PathBuf::from(legacy);
            if p.exists() {
                sources.push(p);
                break;
            }
        }
        if let Some(explicit) = explicit_path {
            if explicit.exists() {
                sources.push(explicit.to_path_buf());
            }
        }

        if sources.is_empty() {
            anyhow::bail!(
                "未找到配置文件。\n\
                 请运行 `cctraveler init` 初始化配置，或在以下任一位置创建 config.toml：\n  \
                 - {}\n  \
                 - ./.cctraveler/config.toml\n  \
                 - ./config.toml",
                user_config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.cctraveler/config.toml".to_string())
            );
        }

        // Merge: later sources override earlier ones at the TOML-table level.
        let mut merged = toml::Value::Table(toml::map::Map::new());
        for src in &sources {
            let content = std::fs::read_to_string(src)?;
            let value: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("解析 {} 失败: {e}", src.display()))?;
            merge_toml(&mut merged, value);
        }

        let config: Self = merged.try_into()?;
        Ok(config)
    }
}

/// `~/.cctraveler/config.toml` — None if HOME is unset.
pub fn user_config_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cctraveler").join("config.toml"))
}

/// Project-level config: `./.cctraveler/config.toml`.
pub fn project_config_path() -> PathBuf {
    PathBuf::from(".cctraveler/config.toml")
}

/// Recursive TOML merge: `b` takes precedence over `a`.
fn merge_toml(a: &mut toml::Value, b: toml::Value) {
    match (a, b) {
        (toml::Value::Table(at), toml::Value::Table(bt)) => {
            for (k, v) in bt {
                match at.get_mut(&k) {
                    Some(av) => merge_toml(av, v),
                    None => {
                        at.insert(k, v);
                    }
                }
            }
        }
        (a_slot, b_val) => {
            *a_slot = b_val;
        }
    }
}
