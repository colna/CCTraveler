// Core types have moved to runtime::types.
// This module is kept for backward compatibility.
// Use runtime types directly.
pub use runtime::types::{ApiRequest, AssistantEvent, ToolSpec};

// Legacy aliases (deprecated)
pub type ToolDefinition = ToolSpec;
