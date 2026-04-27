# T8 · `@file` / `@url` 上下文注入

> 用户输入中的 `@path/to/file` 或 `@https://...` 会被自动展开为内嵌内容，方便边对话边喂资料。

## 完成状态
✅ 实现 + 3 个单元测试通过。

## 思路与逻辑

### 期望体验
```
you> 帮我看下 @./itinerary-draft.md 这份草稿，5/1 到 5/3 上海行程合理吗？
you> 总结一下 @https://en.wikipedia.org/wiki/Shanghai_Metro 这篇地铁介绍
```
两种 token 走同一管线，REPL 收到输入后调用 `expand::expand(input)` 得到展开后的字符串，再喂给 `run_turn`。

### Token 识别
按空白切分；任一 segment 以 `@` 开头视为引用。
- 防止误伤 `someone@example.com` —— 邮箱中的 `@` 不在 token 起始，自然跳过；
- 防止误伤 `@username` 这类社交占位 —— 当作 path 解析时找不到文件，会保留原样并打印一行 `⚠ 无法展开 @username` 警告，不影响 LLM 看到原文。

### URL vs 文件
- `http://` / `https://` 前缀 → 走 `fetch_url`（reqwest blocking + 8s timeout + UA）
- 其他 → 走 `read_file`（支持 `~/...` 展开 HOME）

### 输出包装
展开内容包在
```
<ref src="...">
内容
</ref>
```
中。优点：
- LLM 一眼能区分"用户原文 vs 注入资料"；
- 多个引用可在一段输入里堆叠不混淆；
- 后续 prompt 工程可以教 system prompt 引用这种标签。

### 大小限制 & HTML 简化
- 单段 32 KiB 截断 + `[truncated, original X bytes]` 提示；
- HTML 用极简 `strip_html`：去 `<...>` + 杀 `<script>/<style>` 段 + 解码常见实体 + 折叠空白。**不**追求渲染保真，只求"喂给 LLM 时去掉视觉噪音"。

### Async / Sync 边界
`repl::run` 是同步函��；为不引入 `tokio::block_on` 这种容易爆 panic 的胶水，给 `cli/Cargo.toml` 的 `reqwest` 启 `blocking` feature，URL 抓取走 `reqwest::blocking::Client`。代价是多带一份阻塞 IO 栈，能接受。

### 失败兜底
任何错误 → 保留原 `@xxx` 字符串 + 一行 stderr 警告。**不**让 expand 失败把整个对话搞挂。

## 改动文件
- `crates/cli/src/expand.rs`（新增，含 3 个单测）
- `crates/cli/src/main.rs` — 注册模块
- `crates/cli/src/repl.rs` — 输入预处理调用
- `crates/cli/Cargo.toml` — `reqwest` 加 `blocking` feature

## 测试
```
$ cargo test -p cctraveler --bin cctraveler
running 3 tests
test expand::tests::strip_html_basic       ... ok
test expand::tests::passthrough_when_no_at ... ok
test expand::tests::unknown_file_keeps_token ... ok
```

## 后续
- 支持 `@@` 转义（输出原 `@xxx` 不展开）；
- 抓 PDF / docx 走 [pandoc] 或外部转换（v0.5）；
- URL 抓取做并发预热（`@a @b @c` 串行会慢）。
