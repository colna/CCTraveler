use crate::sse;
use runtime::types::{
    ApiClient, ApiRequest, AssistantEvent, ContentBlock, RuntimeError, TokenUsage,
};
use serde::Deserialize;
use tracing::{debug, info};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API client implementing the `ApiClient` trait.
/// Uses SSE streaming internally, collects all events before returning.
pub struct AnthropicRuntimeClient {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicRuntimeClient {
    #[must_use] 
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::new();
        Self { api_key, client }
    }

    pub fn from_env() -> Result<Self, RuntimeError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| RuntimeError::Api("ANTHROPIC_API_KEY not set".into()))?;
        Ok(Self::new(api_key))
    }

    /// Build the Anthropic API request body.
    fn build_request_body(&self, request: &ApiRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    runtime::MessageRole::User => "user",
                    runtime::MessageRole::Assistant => "assistant",
                };
                let content: Vec<serde_json::Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => serde_json::json!({
                            "type": "text",
                            "text": text
                        }),
                        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            output,
                            is_error,
                        } => {
                            let mut obj = serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": output,
                            });
                            if *is_error {
                                obj["is_error"] = serde_json::json!(true);
                            }
                            obj
                        }
                    })
                    .collect();
                serde_json::json!({ "role": role, "content": content })
            })
            .collect();

        let tools: Vec<serde_json::Value> = request
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "system": request.system,
            "messages": messages,
            "stream": true
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
        }

        body
    }

    /// Send request and collect SSE events.
    async fn stream_async(
        &self,
        request: ApiRequest,
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let body = self.build_request_body(&request);
        debug!("Sending request to Anthropic API");

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RuntimeError::Api(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(RuntimeError::Api(format!(
                "Anthropic API returned {status}: {error_body}"
            )));
        }

        let sse_body = response
            .text()
            .await
            .map_err(|e| RuntimeError::Api(format!("Failed to read response body: {e}")))?;

        self.parse_sse_events(&sse_body)
    }

    /// Parse collected SSE events into `AssistantEvents`.
    fn parse_sse_events(&self, body: &str) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let sse_events = sse::parse_sse_body(body);
        let mut result = Vec::new();

        // Accumulators for content blocks
        let mut block_accumulators: Vec<BlockAccumulator> = Vec::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;

        for event in &sse_events {
            match event.event_type.as_str() {
                "message_start" => {
                    if let Ok(msg) =
                        serde_json::from_str::<MessageStartEvent>(&event.data)
                    {
                        if let Some(usage) = msg.message.usage {
                            input_tokens += usage.input_tokens.unwrap_or(0);
                            output_tokens += usage.output_tokens.unwrap_or(0);
                        }
                    }
                }
                "content_block_start" => {
                    if let Ok(block_start) =
                        serde_json::from_str::<ContentBlockStartEvent>(&event.data)
                    {
                        let acc = match block_start.content_block.r#type.as_str() {
                            "text" => BlockAccumulator::Text {
                                index: block_start.index,
                                text: block_start
                                    .content_block
                                    .text
                                    .unwrap_or_default(),
                            },
                            "tool_use" => BlockAccumulator::ToolUse {
                                index: block_start.index,
                                id: block_start.content_block.id.unwrap_or_default(),
                                name: block_start.content_block.name.unwrap_or_default(),
                                input_json: String::new(),
                            },
                            _ => continue,
                        };
                        block_accumulators.push(acc);
                    }
                }
                "content_block_delta" => {
                    if let Ok(delta) =
                        serde_json::from_str::<ContentBlockDeltaEvent>(&event.data)
                    {
                        if let Some(acc) = block_accumulators
                            .iter_mut()
                            .find(|a| a.index() == delta.index)
                        {
                            match delta.delta.r#type.as_str() {
                                "text_delta" => {
                                    if let BlockAccumulator::Text { text, .. } = acc {
                                        if let Some(t) = &delta.delta.text {
                                            text.push_str(t);
                                        }
                                    }
                                }
                                "input_json_delta" => {
                                    if let BlockAccumulator::ToolUse { input_json, .. } = acc {
                                        if let Some(pj) = &delta.delta.partial_json {
                                            input_json.push_str(pj);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "content_block_stop" => {
                    // Block finalized — will be collected at the end
                }
                "message_delta" => {
                    if let Ok(msg_delta) =
                        serde_json::from_str::<MessageDeltaEvent>(&event.data)
                    {
                        if let Some(stop_reason) = msg_delta.delta.stop_reason {
                            result.push(AssistantEvent::StopReason(stop_reason));
                        }
                        if let Some(usage) = msg_delta.usage {
                            output_tokens += usage.output_tokens.unwrap_or(0);
                        }
                    }
                }
                "message_stop" | "ping" => {}
                other => {
                    debug!("Unknown SSE event type: {other}");
                }
            }
        }

        // Convert accumulators to AssistantEvents
        for acc in block_accumulators {
            match acc {
                BlockAccumulator::Text { text, .. } => {
                    if !text.is_empty() {
                        result.push(AssistantEvent::ContentBlock(ContentBlock::Text { text }));
                    }
                }
                BlockAccumulator::ToolUse {
                    id,
                    name,
                    input_json,
                    ..
                } => {
                    let input: serde_json::Value =
                        serde_json::from_str(&input_json).unwrap_or(serde_json::json!({}));
                    info!("Parsed tool_use: {name}");
                    result.push(AssistantEvent::ContentBlock(ContentBlock::ToolUse {
                        id,
                        name,
                        input,
                    }));
                }
            }
        }

        // Add usage
        result.push(AssistantEvent::Usage(TokenUsage {
            input_tokens,
            output_tokens,
        }));

        Ok(result)
    }
}

impl ApiClient for AnthropicRuntimeClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // Bridge async to sync using block_in_place (requires multi-threaded tokio runtime)
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.stream_async(request))
        })
    }
}

// ─── Anthropic SSE Event Types (internal) ───

#[derive(Debug)]
enum BlockAccumulator {
    Text {
        index: usize,
        text: String,
    },
    ToolUse {
        index: usize,
        id: String,
        name: String,
        input_json: String,
    },
}

impl BlockAccumulator {
    fn index(&self) -> usize {
        match self {
            Self::Text { index, .. } | Self::ToolUse { index, .. } => *index,
        }
    }
}

#[derive(Deserialize)]
struct MessageStartEvent {
    message: MessageStartMessage,
}

#[derive(Deserialize)]
struct MessageStartMessage {
    usage: Option<UsageInfo>,
}

#[derive(Deserialize)]
struct UsageInfo {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ContentBlockStartEvent {
    index: usize,
    content_block: ContentBlockInfo,
}

#[derive(Deserialize)]
struct ContentBlockInfo {
    r#type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlockDeltaEvent {
    index: usize,
    delta: DeltaInfo,
}

#[derive(Deserialize)]
struct DeltaInfo {
    r#type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
}

#[derive(Deserialize)]
struct MessageDeltaEvent {
    delta: MessageDeltaInfo,
    #[serde(default)]
    usage: Option<UsageInfo>,
}

#[derive(Deserialize)]
struct MessageDeltaInfo {
    stop_reason: Option<String>,
}
