use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct RuntimeConfig {
    pub agent: AgentConfig,
    pub scraper: ScraperConfig,
    pub storage: StorageConfig,
    pub ctrip: CtripConfig,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub max_turns: u32,
    #[serde(default)]
    pub api_key: String,
}

impl AgentConfig {
    /// Resolve API key: config file > env var ANTHROPIC_API_KEY.
    pub fn resolve_api_key(&self) -> Option<String> {
        if !self.api_key.is_empty() {
            return Some(self.api_key.clone());
        }
        std::env::var("ANTHROPIC_API_KEY").ok()
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
