use crate::sse;
use futures_util::StreamExt;
use runtime::types::{
    ApiClient, ApiRequest, AssistantEvent, ContentBlock, RuntimeError, TokenUsage,
};
use serde::Deserialize;
use tracing::{debug, info};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API client implementing the `ApiClient` trait.
/// Uses SSE streaming internally, collects all events before returning.
pub struct AnthropicRuntimeClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicRuntimeClient {
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL.to_string())
    }

    #[must_use]
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = reqwest::Client::new();
        let base_url = base_url.trim_end_matches('/').to_string();
        Self { api_key, base_url, client }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
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
        self.stream_async_inner(request, None::<&dyn Fn(&str)>).await
    }

    /// Streaming variant: parses SSE incrementally and invokes `on_text_delta`
    /// for every `text_delta` chunk as it arrives. Final `Vec<AssistantEvent>`
    /// is identical to `stream_async`.
    async fn stream_async_with_text(
        &self,
        request: ApiRequest,
        on_text_delta: &dyn Fn(&str),
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        self.stream_async_inner(request, Some(on_text_delta)).await
    }

    async fn stream_async_inner(
        &self,
        request: ApiRequest,
        on_text_delta: Option<&dyn Fn(&str)>,
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let body = self.build_request_body(&request);
        debug!("Sending request to Anthropic API");

        let response = self
            .client
            .post(self.messages_url())
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

        // Incremental SSE parsing: read bytes_stream, split by "\n\n",
        // parse each event, optionally fire `on_text_delta` for text_delta.
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut sse_events: Vec<sse::SseEvent> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| RuntimeError::Api(format!("Stream read failed: {e}")))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Drain complete events (separator: blank line "\n\n")
            while let Some(idx) = buffer.find("\n\n") {
                let raw = buffer[..idx].to_string();
                buffer.drain(..idx + 2);
                if raw.trim().is_empty() {
                    continue;
                }
                let mut event_type = String::new();
                let mut data = String::new();
                for line in raw.lines() {
                    if let Some(t) = line.strip_prefix("event: ") {
                        event_type = t.trim().to_string();
                    } else if let Some(d) = line.strip_prefix("data: ") {
                        data = d.to_string();
                    }
                }
                if data.is_empty() {
                    continue;
                }

                // Side-effect: fire text_delta callback ASAP.
                if event_type == "content_block_delta" {
                    if let (Some(cb), Ok(delta)) = (
                        on_text_delta.as_ref(),
                        serde_json::from_str::<ContentBlockDeltaEvent>(&data),
                    ) {
                        if delta.delta.r#type == "text_delta" {
                            if let Some(t) = &delta.delta.text {
                                cb(t);
                            }
                        }
                    }
                }

                sse_events.push(sse::SseEvent {
                    event_type,
                    data,
                });
            }
        }

        // Re-use existing aggregator on collected events (re-encode into a body).
        let mut as_body = String::new();
        for ev in &sse_events {
            if !ev.event_type.is_empty() {
                as_body.push_str("event: ");
                as_body.push_str(&ev.event_type);
                as_body.push('\n');
            }
            as_body.push_str("data: ");
            as_body.push_str(&ev.data);
            as_body.push_str("\n\n");
        }
        self.parse_sse_events(&as_body)
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

    fn stream_with_text_delta(
        &mut self,
        request: ApiRequest,
        on_text_delta: &dyn Fn(&str),
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.stream_async_with_text(request, on_text_delta))
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
