# T6 · 斜杠命令体系

> REPL 内引入 `/help` `/clear` `/cost` `/tools` `/model` `/sessions` `/export` `/quit` 等命令。

## 完成状态
✅ 已实现，编译通过，烟雾测试 `/help → /quit` 正确。

## 思路与逻辑

### 设计
- **解析与执行解耦**：`Command::parse(input) -> Option<Command>`，再 `cmd.execute(&mut rt, db_path) -> Action`，便于单测与扩展。
- **优先级**：在 REPL 主循环里，*先* 尝试解析为斜杠命令；命中则吃掉本轮输入不交给 LLM，否则才走 `run_turn`。
- **Action 反馈**：执行结果只有两种语义 —— `Continue`（继续 REPL）/ `Quit`（退出循环）。避��到处用早 return。
- **保留兼容**：旧的裸 `quit`/`exit`/`q`（无斜杠）也仍然退出，避免老用户肌肉记忆中断。

### Runtime 暴露的 accessors
为了让 REPL 能渲染 `/cost` `/tools` `/model`，给 `ConversationRuntime` 加了几个最小 getter / mutator：
| API | 用途 |
|---|---|
| `usage() -> &UsageTracker` | `/cost` 累计 token |
| `model() -> &str` | `/model` 显示当前 |
| `set_model(String)` | `/model <name>` 切换 |
| `tools() -> Vec<ToolSpec>` | `/tools` 列出 |
| `reset_session()` | `/clear` `/new` |

`reset_session` 的实现细节：保留 `workspace_root`，其他字段重新生成 —— 这样新 session 的 jsonl 会写到同一个目录下，且 session_id 是新的（不会覆盖老对话）。

### `/cost` 估算
按 sonnet 4 的参考价（input $3 / output $15 per 1M tokens）做最简估算。注释里写明假设，避免给用户错误的精确感。后续若按模型/Provider 区分，再做查表。

### `/sessions` 实现
直接扫描 `./.cctraveler/sessions/*.jsonl`，按 `mtime` 倒序，最多 20 行。session_id、修改时间、文件大小三列。`--resume <id>` 留到 T9 落地，先在输出里提示用户。

### `/export`
- `md`（默认）：把消息渲染成 Markdown；区分 `User`/`Assistant`、`tool_use` 块用 `json` 代码块、`tool_result` 用纯代码块并截断到 400 字符。
- `json`：直接 `serde_json::to_string_pretty(&rt.session)`，便于二次处理。

输出位置：`./.cctraveler/exports/<session_id>.<ext>`。

## 改动文件
- `crates/cli/src/slash.rs`（新增）
- `crates/cli/src/main.rs` — 注册模块
- `crates/cli/src/repl.rs` — 主循环接入解析
- `crates/runtime/src/conversation.rs` — 新增 5 个 accessor

## 验证
```
$ printf "/help\n/quit\n" | cctraveler
╔════════════════════════════════════════╗
║   CCTraveler AI 旅行助手               ║
║   /help 查看命令，/quit 退出            ║
╚════════════════════════════════════════╝
可用斜杠命令：
  /help, /h, /?       显示本帮助
  /clear, /new        清空当前会话上下文
  /cost               查看 token 用量
  /tools              列出已加载工具
  /model [name]       查看或切换模型
  /sessions           列出历史 session
  /export [md|json]   导出当前会话
  /quit, /exit, /q    退出
再见！
```

## 后续延伸
- `/resume <id>` —— T9
- `/compact` —— 主动压缩，目前是 token 阈值自动触发
- `/system` —— 查看/编辑当前 system prompt
