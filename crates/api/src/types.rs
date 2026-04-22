use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<serde_json::Value>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse {
    pub content: Vec<serde_json::Value>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
