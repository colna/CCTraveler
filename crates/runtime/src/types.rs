use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

// ─── Message Types ───

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        output: String,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

// ─── Session Types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCompaction {
    pub summary: String,
    pub original_message_count: usize,
    pub compacted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPromptEntry {
    pub text: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub messages: Vec<ConversationMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<SessionCompaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub prompt_history: Vec<SessionPromptEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

// ─── Tool Types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct GlobalToolRegistry {
    tools: Vec<ToolSpec>,
}

impl GlobalToolRegistry {
    #[must_use] 
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, spec: ToolSpec) {
        self.tools.push(spec);
    }

    #[must_use] 
    pub fn specs(&self) -> &[ToolSpec] {
        &self.tools
    }

    #[must_use] 
    pub fn to_vec(&self) -> Vec<ToolSpec> {
        self.tools.clone()
    }
}

// ─── API Types ───

#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ConversationMessage>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum AssistantEvent {
    ContentBlock(ContentBlock),
    Usage(TokenUsage),
    StopReason(String),
}

// ─── Turn Summary ───

#[derive(Debug)]
pub struct TurnSummary {
    pub assistant_text: String,
    pub tool_calls_made: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ─── Error Types ───

#[derive(Debug)]
pub enum RuntimeError {
    Api(String),
    Tool { tool_name: String, message: String },
    MaxIterations(usize),
    Session(String),
    Other(anyhow::Error),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api(msg) => write!(f, "API error: {msg}"),
            Self::Tool { tool_name, message } => write!(f, "Tool '{tool_name}': {message}"),
            Self::MaxIterations(n) => write!(f, "Max iterations ({n}) exceeded"),
            Self::Session(msg) => write!(f, "Session error: {msg}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<anyhow::Error> for RuntimeError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

// ─── Permission (simplified: allow-all) ───

pub struct PermissionPolicy;

impl PermissionPolicy {
    #[must_use] 
    pub fn new_allow_all() -> Self {
        Self
    }

    #[must_use] 
    pub fn check(&self, _tool_name: &str) -> bool {
        true
    }
}

// ─── Usage Tracker ───

#[derive(Debug, Default)]
pub struct UsageTracker {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
}

impl UsageTracker {
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, usage: &TokenUsage) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
    }
}

// ─── Hook System (placeholder) ───

#[derive(Debug)]
pub enum HookResult {
    Allow,
    Deny(String),
}

pub struct HookRunner;

impl Default for HookRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRunner {
    #[must_use] 
    pub fn new() -> Self {
        Self
    }

    #[must_use] 
    pub fn pre_tool_use(&self, _tool_name: &str, _input: &str) -> HookResult {
        HookResult::Allow
    }

    #[must_use] 
    pub fn post_tool_use(&self, _tool_name: &str, _output: &str) -> HookResult {
        HookResult::Allow
    }
}

// ─── Tool Observation Listener ───

/// 工具调用事件 —— 只读观察，与 hook 的 allow/deny 决策正交。
#[derive(Debug, Clone)]
pub enum ToolEvent {
    Start {
        name: String,
        input: serde_json::Value,
    },
    Finish {
        name: String,
        ok: bool,
        output_chars: usize,
        elapsed_ms: u64,
    },
}

pub type ToolListener = std::sync::Arc<dyn Fn(&ToolEvent) + Send + Sync>;

/// 文本增量回调：流式 LLM 输出每收到一段 text_delta 就调用。
pub type TextDeltaListener = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

// ─── Traits ───

/// API client trait — abstracts LLM providers (Anthropic, `OpenAI`, etc.)
/// Synchronous interface; async handled internally.
pub trait ApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError>;

    /// 流式版本：每段 text_delta 调用 `on_text_delta`，最终仍返回完整事件序列。
    /// 默认实现 = 调用 `stream`，把最终 text 作为单段 delta 一次性 emit。
    /// 真正的流式实现应 override 该方法。
    fn stream_with_text_delta(
        &mut self,
        request: ApiRequest,
        on_text_delta: &dyn Fn(&str),
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let events = self.stream(request)?;
        for ev in &events {
            if let AssistantEvent::ContentBlock(ContentBlock::Text { text }) = ev {
                on_text_delta(text);
            }
        }
        Ok(events)
    }
}

/// Tool executor trait — dispatches tool calls to handlers.
pub trait ToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, RuntimeError>;
    fn tool_specs(&self) -> Vec<ToolSpec>;
}
