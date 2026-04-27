# T10 · token 流式输出

> Agent 输出文本时，不再等整段返回再 print，而是边收边打到终端，体验对齐 Claude / ChatGPT。

## 完成状态
✅ 编译通过；既有单测全绿。真实流式效果需带 API key 跑 REPL 才能看到，但代码路径已就绪。

## 思路与逻辑

### 现状的"假"流式
原 `AnthropicRuntimeClient::stream_async` 流程：
1. 发请求；
2. `response.text().await` —— **一次性把整个 SSE body 收完**；
3. 才开始 `parse_sse_body` 解析。
等于把流式协议 SSE 当批量协议用了。要做真流式，必须改成边收边解析。

### Trait 不破坏：默认 + override
不动 `ApiClient::stream` 签名（破坏所有调用方，不值得），改为给 trait 加一个**默认方法**：

```rust
fn stream_with_text_delta(
    &mut self,
    request: ApiRequest,
    on_text_delta: &dyn Fn(&str),
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    // 默认 fallback：调老的 stream，最后把整段 text 当成单段 delta 一次性 emit
    let events = self.stream(request)?;
    for ev in &events {
        if let AssistantEvent::ContentBlock(ContentBlock::Text { text }) = ev {
            on_text_delta(text);
        }
    }
    Ok(events)
}
```
任何不重写它的 provider（OpenAI 兼容等）都自动获得"伪流式"，不阻塞功能上线。

### 真正的流式实现
`AnthropicRuntimeClient` override：
1. `bytes_stream()` 异步迭代 chunk；
2. 维护一个增长 `String` buffer，按 `"\n\n"` 切出完整 SSE 事件；
3. 对每个事件：
   - `event_type == "content_block_delta"` 且 `delta.type == "text_delta"` → **立刻**调 `on_text_delta(text)`；
   - 同时把事件原样 push 到 sse_events 列表；
4. 流结束后把 sse_events 重新拼回一段标准 SSE body，复用既有 `parse_sse_events` 做最终聚合（生成完整 ContentBlock 序列、Usage、StopReason）。

> **复用 `parse_sse_events`** 是关键工程决策：避免重复实现 60 行的累加器逻辑，且让两个路径产生**完全相同**的 final event vector，保证 `ConversationRuntime` 里的 tool 调度逻辑零变化。

### Runtime 注入与 REPL 接入
- `runtime/types.rs` 新增 `TextDeltaListener = Arc<dyn Fn(&str) + Send + Sync>`；
- `ConversationRuntime` 加字段 + `set_text_listener`；
- `run_turn` 里若 listener 存在 → 调 `stream_with_text_delta`，否则走老路（一次性）；
- REPL 注册：
  ```rust
  rt.set_text_listener(Arc::new(|delta| {
      print!("{delta}");
      let _ = std::io::stdout().flush();
  }));
  ```
- 输出布局调整：在 `run_turn` 前先打 `\nassistant> ` 前缀并 flush，token 直接接在后面流；run_turn 完成后只补换行 + footer。

### 为什么 tool_use 不流式？
LLM 的 `tool_use` 块 input 是 JSON，必须等完整才能 parse，部分 chunk 没意义。本实现只流 text；tool_use 仍按现有"完整收集后一次性 emit"的方式。这正好与 T7 的 `ToolEvent::Start/Finish` 协同——用户先看到流式回复，再看到 ⏳ 工具列表，逻辑清晰。

### Async/Sync 桥
保持原 `tokio::task::block_in_place + Handle::current().block_on` 桥接（CLI 是 multi-thread runtime）。

### 依赖变化
`crates/api/Cargo.toml`：
- `reqwest` 启用 `stream` feature
- 新增 `futures-util = "0.3"`（拿 `StreamExt::next()`）

## 改动文件
- `crates/runtime/src/types.rs` — `TextDeltaListener` + trait 默认方法
- `crates/runtime/src/lib.rs` — re-export
- `crates/runtime/src/conversation.rs` — 字段 + setter + `stream_with_text_delta` 调用
- `crates/api/Cargo.toml` — features + futures-util
- `crates/api/src/providers/anthropic.rs` — 拆 `stream_async_inner` + 增量 SSE 解析 + override trait
- `crates/cli/src/repl.rs` — 注册 listener + 调整输出布局

## 验证
- `cargo build -p cctraveler` ✔
- `cargo test -p cctraveler` ✔（3 passed）
- 真实流式效果需 API key 跑 `cctraveler` 与 LLM 对话观察。

## 已知 trade-off
- 流式期间若 LLM 同时输出 text 和 tool_use（罕见但 Anthropic 会），text 流出而 tool_use 等到末尾——用户感知是"先文字回答，再看到调用工具"，符合自然预期；
- `print!` 直接写 stdout，未做 ANSI 转义清洗——LLM 输出原始 markdown 字符（如 \`\`\`）会原样显示。后续可加 streaming markdown renderer（v0.5）。
