# T1 · 默认进对话（裸命令进 REPL）

> 让 `cctraveler` 输入回车后直接进入 AI 对话，等价于 `cctraveler chat`。

## 完成状态
✅ 已实现并通过编译 / `--help` / `doctor` 烟雾测试。

## 思路与逻辑

### 问题
原 `crates/cli/src/main.rs` 用 clap 的 `#[command(subcommand)] command: Commands` 强制要求子命令，裸跑会报错。所有逻辑（chat REPL、scrape、search、export）也全部塞在 main.rs 一个文件里，扩展性差。

### 方案
1. **clap 改造**：把 `command: Commands` 改成 `command: Option<Commands>`。匹配时：
   - `Some(Chat)` 或 `None`（且无 `-p`）→ 走 REPL；
   - `Some(其他)` → 老命令；
   - `None + -p` → 一次性模式（T2）。
2. **拆分模块**：把单文件 main.rs 拆成 5 个清晰职责的小文件：
   - `main.rs` — 仅做 clap 解析 + 路由调度
   - `repl.rs` — 交互式 REPL（搬迁原 `run_chat`）
   - `oneshot.rs` — `-p` 一次性模式
   - `commands.rs` — scrape/search/export
   - `init.rs` / `doctor.rs` — 新增子命令
3. **保持 100% 向后兼容**：`scrape/search/export/chat` 行为零变化，老用户脚本不受影响。

### 关键代码片段
```rust
match (cli.command, cli.prompt) {
    (Some(Commands::Chat), _) | (None, None) => repl::run(&config, &db_path)?,
    (None, Some(p))                          => oneshot::run(&config, &db_path, Some(p))?,
    (Some(Commands::Scrape{..}), _)          => commands::scrape(...).await?,
    ...
}
```

## 改动文件
- `crates/cli/src/main.rs`（重写）
- `crates/cli/src/repl.rs`（新增，搬迁 `run_chat`）
- `crates/cli/src/commands.rs`（新增，搬迁 scrape/search/export）

## 验证
```
$ cctraveler --help
Usage: cctraveler [OPTIONS] [COMMAND]
Commands: chat | init | doctor | scrape | search | export
```
裸命令路��已接到 `repl::run`，等价于历史 `cctraveler chat`。
