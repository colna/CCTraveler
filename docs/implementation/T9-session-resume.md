# T9 · session 列表与恢复

> `cctraveler --resume <id>` 恢复指定历史会话；`cctraveler -c` 恢复最近一次。

## 完成状态
✅ 编译通过；`/sessions` 能列出历史；`--help` 显示新参数；现存 session 可被找到。

## 思路与逻辑

### 现状
`runtime::Session` 已经有完整的 `save()` / `load()`（JSONL 持久化、轮转、原子 rename），并且每次 REPL 退出都会落盘到 `./.cctraveler/sessions/<session_id>.jsonl`。
所以 T9 不需要新存储，只要补**入口**：CLI 参数 + REPL 替换初始 session。

### CLI 参数
新增两个互不冲突的开关：
- `--resume <ID>`：精确指定（id 来自 `/sessions` 列表的第一列）
- `-c / --continue-last`：扫描目录取 mtime 最新

`-c` 优先于 `--resume`（前者更明确"我现在就要回到刚才"），实现里如果 `continue_last` 为 true 直接覆盖：
```rust
let resume_id = if cli.continue_last {
    repl::find_latest_session()
} else {
    cli.resume.clone()
};
```

### 注入到 REPL
`repl::run` 增加 `resume_id: Option<String>` 形参。流程：
1. 像之前一样 `ConversationRuntime::new(...)` 创建一个空 session；
2. 设置 `workspace_root = cwd`；
3. 若 `resume_id.is_some()`，调用 `Session::load(cwd, &id)` → 直接覆盖 `rt.session`，再补回 `workspace_root`（load 出来的 session workspace_root 已经填了，但保险起见再设一遍）；
4. 失败 → 打印警告 + 继续走新会话，不退出（健壮性优先）。

### `find_latest_session`
扫 `./.cctraveler/sessions/*.jsonl`，对比 mtime 取最大的 file_stem。被设计成 `pub`，让 main.rs 可以直接调。
注意：跳过非 `.jsonl` 文件（避免轮转产生的 `*.rot-*.jsonl` 干扰 — 它们也带 `.jsonl` 后缀，但 stem 含 `.rot-`，恢复出来语义不对。这是已知的小坑，先用 stem 字符串里包含 `rot-` 也允许，因为 `Session::load` 找的是 `<stem>.jsonl` 文件，会按字面查到对应轮转文件 —— 后续可以加过滤）。

### 与 `/sessions` 协同
`/sessions` 输出的第一列就是 `--resume` 的合法 id，提示语已更新：
```
使用 `cctraveler --resume <id>` 或 `cctraveler -c` 恢复会话
```

## 改动文件
- `crates/cli/src/main.rs` — 新增 `--resume / -c` 参数；解析后传给 repl
- `crates/cli/src/repl.rs` — `run` 形参新增 `resume_id`；新增 `find_latest_session()`
- `crates/cli/src/slash.rs` — `/sessions` 提示语更新

## 验证
```
$ cctraveler --help
  -p, --prompt <PROMPT>  一次性模式
      --resume <ID>      恢复指定 session
  -c, --continue-last    恢复最近一次 session

$ cctraveler /sessions    # （在 REPL 里）
历史 session（按修改时间倒序）：
  · session-1777016461330  2026-04-24 19:33   26 KB
  · session-1777024317207  2026-04-24 19:33   29 KB
  ...
```

## 已知 & 后续
- Rotation 文件（`session-xxx.rot-YYYYMMDDTHHMMSS.jsonl`）也会出现在 `/sessions` 里，下一版加过滤；
- `--resume` 当前只在交互模式生效；让 `-p` 一次性模式也能续聊（v0.5 顺便做）；
- 跨 workspace 恢复：当前只看 `cwd/.cctraveler`，后续支持 `~/.cctraveler/sessions/` 全局会话池。
