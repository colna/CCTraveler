use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct RuntimeConfig {
    pub agent: AgentConfig,
    pub scraper: ScraperConfig,
    pub storage: StorageConfig,
    pub ctrip: CtripConfig,
    #[serde(default)]
    pub redis: RedisConfig,
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

impl RuntimeConfig {
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
}
