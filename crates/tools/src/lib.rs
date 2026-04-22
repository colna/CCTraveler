pub mod scrape;
pub mod search;
pub mod analyze;
pub mod export;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
}

/// Tool executor trait — dispatches tool calls to handlers.
pub trait ToolExecutor {
    fn execute(&self, tool_name: &str, input: &str) -> anyhow::Result<String>;
}
