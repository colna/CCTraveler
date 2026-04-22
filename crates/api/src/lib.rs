pub mod providers;
pub mod sse;

// Re-export the Anthropic client as the primary provider
pub use providers::anthropic::AnthropicRuntimeClient;

// Re-export core traits from runtime for convenience
pub use runtime::{ApiClient, ApiRequest, AssistantEvent, RuntimeError, ToolSpec};
