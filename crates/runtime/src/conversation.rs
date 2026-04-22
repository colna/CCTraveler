use crate::types::{
    ApiClient, ApiRequest, AssistantEvent, ContentBlock, ConversationMessage, HookResult,
    HookRunner, MessageRole, PermissionPolicy, RuntimeError, Session, TokenUsage, ToolExecutor,
    TurnSummary, UsageTracker,
};
use tracing::{info, warn};

/// `ConversationRuntime`<C, T> — the core Agent loop.
///
/// Generic over:
/// - C: `ApiClient` (LLM provider)
/// - T: `ToolExecutor` (tool dispatch)
///
/// Directly follows the claw-code `ConversationRuntime` pattern.
pub struct ConversationRuntime<C, T> {
    pub session: Session,
    api_client: C,
    tool_executor: T,
    permission_policy: PermissionPolicy,
    system_prompt: String,
    max_iterations: usize,
    usage_tracker: UsageTracker,
    hook_runner: HookRunner,
    auto_compaction_threshold: u32,
    model: String,
}

impl<C: ApiClient, T: ToolExecutor> ConversationRuntime<C, T> {
    pub fn new(
        api_client: C,
        tool_executor: T,
        model: String,
        system_prompt: String,
        max_iterations: usize,
    ) -> Self {
        Self {
            session: Session::new(Some(model.clone())),
            api_client,
            tool_executor,
            permission_policy: PermissionPolicy::new_allow_all(),
            system_prompt,
            max_iterations,
            usage_tracker: UsageTracker::new(),
            hook_runner: HookRunner::new(),
            auto_compaction_threshold: 100_000,
            model,
        }
    }

    /// Run a single turn: user input → LLM → (tool loop) → assistant text.
    pub fn run_turn(&mut self, user_input: &str) -> Result<TurnSummary, RuntimeError> {
        // Record user prompt
        self.session.push_user_prompt(user_input);

        // Push user message into session
        let user_msg = ConversationMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: user_input.to_string(),
            }],
            usage: None,
        };
        self.session.push_message(user_msg);

        let mut total_tool_calls = 0;
        let mut turn_input_tokens = 0u32;
        let mut turn_output_tokens = 0u32;
        let mut assistant_text = String::new();

        for iteration in 0..self.max_iterations {
            info!("Agent iteration {}/{}", iteration + 1, self.max_iterations);

            // Build API request
            let request = ApiRequest {
                model: self.model.clone(),
                system: self.system_prompt.clone(),
                messages: self.session.messages.clone(),
                tools: self.tool_executor.tool_specs(),
                max_tokens: 4096,
            };

            // Call LLM
            let events = self.api_client.stream(request)?;

            // Process events
            let mut content_blocks = Vec::new();
            let mut usage = TokenUsage::default();
            let mut stop_reason = String::new();

            for event in events {
                match event {
                    AssistantEvent::ContentBlock(block) => content_blocks.push(block),
                    AssistantEvent::Usage(u) => {
                        usage.input_tokens += u.input_tokens;
                        usage.output_tokens += u.output_tokens;
                    }
                    AssistantEvent::StopReason(r) => stop_reason = r,
                }
            }

            turn_input_tokens += usage.input_tokens;
            turn_output_tokens += usage.output_tokens;
            self.usage_tracker.add(&usage);

            // Extract text from content blocks
            for block in &content_blocks {
                if let ContentBlock::Text { text } = block {
                    if !assistant_text.is_empty() {
                        assistant_text.push('\n');
                    }
                    assistant_text.push_str(text);
                }
            }

            // Push assistant message into session
            let assistant_msg = ConversationMessage {
                role: MessageRole::Assistant,
                content: content_blocks.clone(),
                usage: Some(usage),
            };
            self.session.push_message(assistant_msg);

            // Check for tool calls
            let tool_uses: Vec<_> = content_blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.clone()))
                    }
                    _ => None,
                })
                .collect();

            if tool_uses.is_empty() || stop_reason != "tool_use" {
                // No tool calls — turn complete
                info!("Turn complete: stop_reason={stop_reason}");
                break;
            }

            // Execute tool calls
            let mut tool_results = Vec::new();
            for (tool_id, tool_name, tool_input) in &tool_uses {
                total_tool_calls += 1;
                info!("Tool call: {tool_name}({tool_input})");

                // Pre-tool hook
                let input_str = tool_input.to_string();
                if let HookResult::Deny(reason) =
                    self.hook_runner.pre_tool_use(tool_name, &input_str)
                {
                    warn!("Hook denied tool {tool_name}: {reason}");
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: tool_id.clone(),
                        output: format!("Tool denied by hook: {reason}"),
                        is_error: true,
                    });
                    continue;
                }

                // Permission check
                if !self.permission_policy.check(tool_name) {
                    warn!("Permission denied for tool {tool_name}");
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: tool_id.clone(),
                        output: format!("Permission denied for tool: {tool_name}"),
                        is_error: true,
                    });
                    continue;
                }

                // Execute
                match self.tool_executor.execute(tool_name, &input_str) {
                    Ok(output) => {
                        // Post-tool hook
                        if let HookResult::Deny(reason) =
                            self.hook_runner.post_tool_use(tool_name, &output)
                        {
                            warn!("Post-hook denied tool {tool_name}: {reason}");
                            tool_results.push(ContentBlock::ToolResult {
                                tool_use_id: tool_id.clone(),
                                output: format!("Tool result rejected by hook: {reason}"),
                                is_error: true,
                            });
                        } else {
                            info!("Tool {tool_name} succeeded ({} chars)", output.len());
                            tool_results.push(ContentBlock::ToolResult {
                                tool_use_id: tool_id.clone(),
                                output,
                                is_error: false,
                            });
                        }
                    }
                    Err(e) => {
                        warn!("Tool {tool_name} failed: {e}");
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: tool_id.clone(),
                            output: format!("Error: {e}"),
                            is_error: true,
                        });
                    }
                }
            }

            // Push tool results as a user message (Anthropic API convention)
            let tool_msg = ConversationMessage {
                role: MessageRole::User,
                content: tool_results,
                usage: None,
            };
            self.session.push_message(tool_msg);
        }

        // Auto-compaction check
        if self.usage_tracker.total_input_tokens >= self.auto_compaction_threshold {
            info!(
                "Auto-compaction triggered at {} input tokens",
                self.usage_tracker.total_input_tokens
            );
            self.compact_session();
        }

        Ok(TurnSummary {
            assistant_text,
            tool_calls_made: total_tool_calls,
            input_tokens: turn_input_tokens,
            output_tokens: turn_output_tokens,
        })
    }

    /// Compact the session: summarize old messages, keep recent ones.
    fn compact_session(&mut self) {
        let msg_count = self.session.messages.len();
        if msg_count <= 4 {
            return;
        }

        // Keep the last 4 messages, summarize the rest
        let old_messages = &self.session.messages[..msg_count - 4];
        let recent_messages = self.session.messages[msg_count - 4..].to_vec();

        // Build summary from old messages
        let mut summary_parts = Vec::new();
        let mut tool_names = std::collections::HashSet::new();

        for msg in old_messages {
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        let truncated: String = text.chars().take(100).collect();
                        summary_parts.push(format!("[{:?}] {truncated}", msg.role));
                    }
                    ContentBlock::ToolUse { name, .. } => {
                        tool_names.insert(name.clone());
                    }
                    ContentBlock::ToolResult { .. } => {}
                }
            }
        }

        let tools_used = tool_names.into_iter().collect::<Vec<_>>().join(", ");
        let summary = format!(
            "Session summary ({msg_count} messages compacted to 4):\n\
             Tools used: {tools_used}\n\
             Timeline:\n{}",
            summary_parts.join("\n")
        );

        // Truncate summary to budget
        let summary = if summary.len() > 1200 {
            format!("{}...", &summary[..1200])
        } else {
            summary
        };

        self.session.compaction = Some(crate::types::SessionCompaction {
            summary,
            original_message_count: msg_count,
            compacted_at: chrono::Utc::now().to_rfc3339(),
        });
        self.session.messages = recent_messages;

        info!("Session compacted: {msg_count} → 4 messages");
    }

    /// Save the current session to disk.
    pub fn save_session(&self) -> Result<(), RuntimeError> {
        if let Some(workspace) = &self.session.workspace_root {
            self.session
                .save(workspace)
                .map_err(|e| RuntimeError::Session(e.to_string()))?;
        }
        Ok(())
    }
}
