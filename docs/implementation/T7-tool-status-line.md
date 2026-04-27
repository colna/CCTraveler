# T7 · 工具调用实时状态行

> Agent 调用工具时，REPL 实时打印 `⏳ tool_name(args)` 与完成后的 `✓ tool_name (elapsed, size)`。

## 完成状态
✅ 实现并编译通过。

## 思路与逻辑

### 为什么不直接复用 HookRunner？
`HookRunner.pre_tool_use / post_tool_use` 已经是工具执行点的回调，但其语义是 **决策**（Allow / Deny），混入"观察"职责会让 deny 路径上的副作用难以推理。
所以另开了一个**正交**的 Listener 通道：纯只读，永远不影响执行流。

### 类型设计（runtime crate）
```rust
pub enum ToolEvent {
    Start  { name: String, input: serde_json::Value },
    Finish { name: String, ok: bool, output_chars: usize, elapsed_ms: u64 },
}
pub type ToolListener = Arc<dyn Fn(&ToolEvent) + Send + Sync>;
```
- `Arc<dyn Fn>` 而非 `Box<dyn FnMut>`：避免 listener 持锁；多 token 流式回调时也能从其他 task 调用。
- `&ToolEvent`：listener 不接管事件所有权，方便上层既打印又记录到 metrics。

### 接入位置
`ConversationRuntime` 里：
- 字段 `tool_listener: Option<ToolListener>`
- pub `set_tool_listener(...)`
- 私有 `emit_tool(ev)` 在 4 个分支统一调用：
  - Start：每次工具执行开始
  - Finish (hook deny / permission deny / executor err / executor ok-but-post-deny / ok)

5 条 Finish 路径都覆盖到，避免出现"开始了但没看到结束"的假死观感。

### REPL listener
注入闭包：
```
⏳ search_trains(from="上海", to="北京", date="2026-05-01")
✓ search_trains  (1342ms, 4821 chars)
```
- `summarize_input` 把 JSON 入参拍平成 `key=value` 短串，截断 60 字符；字符串值再单独截 20 字符（避免一个长 query 把单行撑爆）。
- 输出走 `println!`（stdout）：rustyline 的 readline 此刻已经返回，主循环正在等 LLM，不会有输入冲突。

### 兼容性
`ConversationRuntime::new` 默认 `tool_listener = None`，未注册时 `emit_tool` 直接 no-op。所有不接 REPL 的调用方（如 `oneshot`）零成本。

## 改动文件
- `crates/runtime/src/types.rs` — 新增 `ToolEvent` / `ToolListener`
- `crates/runtime/src/lib.rs` — re-export
- `crates/runtime/src/conversation.rs` — 字段 + setter + 4 处 emit + 计时
- `crates/cli/src/repl.rs` — 注册 listener + `summarize_input` 渲染

## 体验示例（预期）
```
you> 帮我查 5 月 1 号上海到北京的高铁
  ⏳ search_trains(from_city="上海", to_city="北京", date="2026-05-..)
  ✓ search_trains  (1342ms, 4821 chars)

assistant> 5 月 1 日上海至北京可选车次：...
  [工具调用: 1 次 | tokens: 1280 in / 612 out]
```

## 后续延伸
- 改成 stderr + carriage-return 单行刷新（spinner 动画），需要让 rustyline 让出终端 raw mode；
- 颜色：⏳ 黄、✓ 绿、✗ 红 —— 引入 `nu-ansi-term` 或 `crossterm::style`；
- 慢工具（>5s）发提醒。
