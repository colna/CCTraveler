# CCTraveler CLI 设计方案

> 目标：像 `claude` 一样，用户安装后直接在终端输入 `cctraveler`，回车即可进入与旅行 Agent 的自然语言对话。

本文档面向产品形态、安装分发、交互设计、技术实现、迁移路径五个维度，给出可落地的设计方案。

---

## 1. 背景与现状

### 1.1 仓库现状（扫描结论）

- **产品定位**：CCTraveler 是一个 AI 旅行规划平台 —— 酒店（携程 / Trip.com）、火车（12306）、机票、城市地理、价格监控、行程规划。
- **技术栈**：Rust workspace（5 crates）+ Python FastAPI scraper + SQLite + 可选 Redis + Next.js dashboard。
- **Agent 内核**：`crates/runtime/src/conversation.rs` 中的 `ConversationRuntime<C: ApiClient, T: ToolExecutor>`，泛型 + 流式 SSE + tool-use 循环。
- **工具系统**：`crates/tools` 注册了 13 个工具（v0.1–v0.3），涵盖酒店、火车、机票、城市、距离、监控、wiki、行程规划。
- **现有 CLI**：`crates/cli/src/main.rs` 编出二进制 `cctraveler`，已有四个子命令：
  - `cctraveler chat` —— 基于 rustyline 的 REPL，已经能跑 Agent 对话
  - `cctraveler scrape / search / export` —— 直接命令式工具

### 1.2 与目标的差距

| 维度 | 现状 | 目标（类 claude code） |
|---|---|---|
| 入口体验 | 必须 `cctraveler chat` | 直接 `cctraveler` 回车进入对话 |
| 安装分发 | `cargo build` 后手动跑 | 一键安装：`cargo install` / `brew` / 预编译二进制 / npm 包装 |
| 首次配置 | 手动复制 `config.toml.example` 填 API key | 启动时检测，引导式 `init` 向导 |
| 流式渲染 | `run_turn` 阻塞拿完整结果再 println | token-by-token 流式打印（含 markdown / tool 调用进度） |
| 斜杠命令 | 仅 `quit/exit/q` | `/help`、`/clear`、`/model`、`/session`、`/cost`、`/config` 等 |
| 会话管理 | 单次启动一个 session，退出时保存 | 多 session 列表 + 恢复 + `--continue` / `--resume <id>` |
| 工具可见性 | 仅打印 "工具调用: N 次" | 实时显示当前工具名 + 参数摘要 + 状态（pending/running/done） |
| 上下文引用 | 无 | `@file`、`@url` 注入，`#` 添加便签 |
| 非交互模式 | 无 | `cctraveler -p "帮我查上海到北京最便宜的高铁"` 一行出结果（管道友好） |
| 项目级配置 | 全局 `config.toml` | 全局 + 项目级 `.cctraveler/config.toml` 叠加 |

---

## 2. 产品形态

### 2.1 核心交互（一句话）

```bash
$ cctraveler
╭─ CCTraveler v0.3.0 · claude-sonnet-4 ────────────────╮
│ 你好，我是你的旅行助手。可以帮你查酒店/火车/机票、    │
│ 规划行程、监控价格。输入 /help 查看命令。              │
╰───────────────────────────────────────────────────────╯
› 帮我查 5 月 1 号上海到北京的高铁，要二等座 500 块以内
```

### 2.2 三种使用模式

| 模式 | 命令 | 场景 |
|---|---|---|
| **交互模式（默认）** | `cctraveler` | 类 claude 对话，长会话、多轮规划 |
| **一次性模式** | `cctraveler -p "..."` 或 `echo "..." \| cctraveler` | 脚本集成、CI、快速查询 |
| **直命令模式（保留）** | `cctraveler scrape / search / export` | 老用户兼容、批处理 |

> 直命令模式作为 `subcommand` 保留；当不传 subcommand 时进入交互模式（即"默认子命令 = chat"）。

---

## 3. 安装分发方案

按优先级实现：

### 3.1 P0 · `cargo install`（最快上线）

```bash
cargo install cctraveler
```

要求：
- 把 `crates/cli` 改名为 `cctraveler-cli`，发布到 crates.io；或保留路径但 `Cargo.toml` 中 `[package] name = "cctraveler"`；
- 可执行文件名 `cctraveler`（已是）；
- 依赖 `bundled` 的 sqlite（已配置 `rusqlite = { features = ["bundled"] }`）。

### 3.2 P0 · 预编译二进制 + 一行安装脚本（类 claude code）

```bash
curl -fsSL https://cctraveler.dev/install.sh | sh
```

- GitHub Actions 矩阵构建：`darwin-arm64 / darwin-x64 / linux-x64 / linux-arm64 / windows-x64`；
- Release tag 触发上传到 GitHub Releases；
- `install.sh` 脚本：探测平台 → 下载 tarball → 解压到 `~/.local/bin/cctraveler` → 提示加 PATH。

### 3.3 P1 · Homebrew tap

```bash
brew install colna/tap/cctraveler
```

发版时自动 PR 更新 tap 仓库（`brew bump-formula-pr`）。

### 3.4 P2 · npm 包装（可选，与 claude code 一致）

```bash
npm install -g cctraveler
```

`packages/cctraveler-npm/` 提供薄壳：`postinstall` 下载对应平台二进制到 node_modules，并在 `bin` 字段暴露。优势：JS 用户群体安装心智一致。

### 3.5 配套：scraper 服务

CLI 依赖 `services/scraper` (FastAPI)。两种方案：

- **托管模式（默认）**：CLI 默认指向官方托管的 scraper endpoint，开箱即用；
- **本地模式**：`cctraveler scraper start` 自动 `docker run` 或 `uv run` 起本地服务，写入 `~/.cctraveler/config.toml`。

---

## 4. 交互设计（CLI 体验）

### 4.1 启动流程

```
cctraveler
  │
  ├─ 读取 ~/.cctraveler/config.toml (全局)
  ├─ 读取 ./.cctraveler/config.toml (项目级，覆盖全局)
  ├─ 检测 API key？
  │    └─ 缺 → 自动跳入 `cctraveler init` 向导（交互式输入）
  ├─ 检测 scraper 可达？
  │    └─ 不可达 → 提示并降级（部分工具不可用）
  └─ 渲染 banner → 进入 REPL
```

### 4.2 REPL 顶层结构（建议升级到 ratatui 或保持 rustyline + ANSI）

**最小可行（rustyline + ANSI 流式）**：

```
╭─ CCTraveler · /Users/me/trip-2026 ────────── claude-sonnet-4 ─╮
│ ① 输入区（多行支持，Shift+Enter 换行，Enter 发送）           │
╰───────────────────────────────────────────────────────────────╯
② 流式输出区（token-by-token，markdown 高亮）
③ 工具状态行（运行中：search_trains  上海→北京  …）
④ 状态栏：tokens 1.2k/4k · cost $0.03 · session abc123
```

**进阶（ratatui 全屏 TUI）**：左栏 session 列表 / 右栏对话 / 底部输入；放到 v0.4。

### 4.3 斜杠命令（与 claude code 对齐 + 旅行域扩展）

| 命令 | 含义 |
|---|---|
| `/help` | 列出所有命令 |
| `/clear` | 清空当前会话上下文（保留磁盘记录） |
| `/new` | 开新 session |
| `/sessions` / `/resume <id>` | 列出/恢复历史 session |
| `/model <name>` | 切换模型（sonnet/opus/haiku） |
| `/cost` | 显示 token 使用与累计费用 |
| `/config` | 打开 config.toml |
| `/init` | 写入项目级 `.cctraveler/CCTRAVELER.md`（项目偏好，类似 CLAUDE.md） |
| `/tools` | 列出已加载工具及调用次数 |
| `/scraper status` | 检查 scraper 服务健康 |
| `/export <md\|json>` | 导出当前会话 |
| `/quit` (`q`, `exit`) | 退出 |

**特殊语法**：
- `@path/to/file` — 把文件内容塞进上下文
- `@https://...` — 抓取 URL ��本（走现有 wiki 工具）
- `#某条便签` — 加入到 session memory

### 4.4 命令行参数（顶层 Cli）

```
cctraveler [OPTIONS] [SUBCOMMAND]

OPTIONS:
  -p, --prompt <TEXT>       一次性模式，直接拿一个回答后退出
      --resume <SESSION>    恢复指定 session
  -c, --continue            恢复最近一次 session
      --model <NAME>        覆盖模型
      --config <PATH>       指定配置文件（默认 ~/.cctraveler/config.toml）
      --no-stream           关闭流式输出
      --json                结构化输出（脚本场景）
  -v, --verbose             显示工具调用细节
  -h, --help / -V, --version

SUBCOMMANDS（保留兼容）:
  chat       显式进入对话（= 不传 subcommand）
  init       配置向导
  scrape     批量抓酒店
  search     查询本地数据库
  export     导出
  scraper    管理本地 scraper 服务
  doctor     诊断环境（API key / scraper / db / 网络）
  mcp        管理 MCP server（预留）
```

> **关键改动**：clap 的 `command` 改为 `Option<Commands>`；当 `None` 时调用 `run_chat`。

---

## 5. 技术实现要点

### 5.1 默认进入对话（最小 diff）

`crates/cli/src/main.rs`：

```rust
#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,

    /// 一次性模式
    #[arg(short = 'p', long)]
    prompt: Option<String>,

    /// 恢复 session
    #[arg(long)]
    resume: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

match (cli.command, cli.prompt) {
    (None, None)       => run_chat(&config, &db_path, cli.resume)?,
    (None, Some(p))    => run_oneshot(&config, &db_path, &p).await?,
    (Some(cmd), _)     => dispatch(cmd, &config, &db_path).await?,
}
```

### 5.2 流式输出

`AnthropicRuntimeClient` 已经在用 SSE，但 `ConversationRuntime::run_turn` 是阻塞接口。改造：

- 新增 `run_turn_streaming(&mut self, input: &str, on_event: impl FnMut(StreamEvent))`；
- `StreamEvent` 枚举：`TextDelta(String) / ToolStart{name, input} / ToolEnd{result} / TurnEnd{summary}`；
- CLI 端用 `crossterm` 把 delta 直接打到 stdout，markdown 用 `termimad` 或自写简易渲染（标题/代码块/列表足够）。

### 5.3 斜杠命令调度

在 REPL 输入循环里先解析：

```rust
if let Some(cmd) = SlashCommand::parse(input) {
    cmd.execute(&mut rt, &mut session_store)?;
    continue;
}
```

`SlashCommand::parse` 把 `/foo bar baz` 切成 enum + args；每个命令实现 `execute`。便于后续插件化。

### 5.4 配置加载层级

```
1. CLI 参数（--model 等）           ← 最高优先级
2. 环境变量（ANTHROPIC_API_KEY 等）
3. ./.cctraveler/config.toml        ← 项目级
4. ~/.cctraveler/config.toml        ← 用户级
5. 内置默认值
```

`runtime::RuntimeConfig::load` 改为 `load_layered(cli_overrides)` 完成 merge。

### 5.5 Session 持久化

复用现有 `rt.session.workspace_root` 与 `rt.save_session()`。新增：

- 目录约定：`~/.cctraveler/sessions/<uuid>.json`（项目级则放 `./.cctraveler/sessions/`）；
- 每条 session 元信息：`id / created_at / model / cwd / first_user_msg / token_total`；
- `/sessions` 读取目录列表，`/resume <id>` 反序列化进 `ConversationRuntime`；
- `--continue` 取 mtime 最新的一条。

### 5.6 工具调用可视化

`TravelerToolExecutor` 添加 `execution_listener: Option<Arc<dyn Fn(ToolEvent)>>`：

```
search_trains  ⠋ 查询中…
search_trains  ✓ 12 班车次（1.3s · 缓存命中）
```

（已经有 `pre_hook / post_hook`，最低改造直接复用。）

### 5.7 首次启动向导（`cctraveler init`）

```
$ cctraveler
未检测到配置，开始初始化…
? Anthropic API key (或留空使用 ANTHROPIC_API_KEY 环境变量): █
? 默认模型: › claude-sonnet-4 / claude-opus-4 / claude-haiku-4
? Scraper 服务: › 官方托管 / 本地启动 / 自定义 URL
? 写入位置: › ~/.cctraveler/config.toml
✔ 完成。开始对话吧！
```

实现：`dialoguer` crate（与 rustyline 不冲突）。

### 5.8 Doctor 子命令

`cctraveler doctor` 检查：API key 有效性 / scraper /healthz / sqlite 可写 / Redis 可达 / 网络出口；输出彩色 ✔/✗ 列表。出问题时退出码非 0，方便 CI。

### 5.9 包名 / 二进制名

- crate name: `cctraveler`（占位，去 crates.io 检查）
- bin name: `cctraveler`（已是）
- 备用入口：`cct`（短别名，可在安装脚本里 `ln -s`）

---

## 6. 路线图（建议三个迭代）

### v0.3.1 · "默认进对话" （1 周内）
- [ ] CLI 改造：`Commands` 改 `Option<Commands>`，无子命令时调 `run_chat`
- [ ] 新增 `-p / --prompt` 一次性模式
- [ ] 新增 `init` / `doctor` 子命令
- [ ] 配置加载支持 `~/.cctraveler/config.toml`
- [ ] 安装脚本 `install.sh` + GitHub Actions 矩阵构建
- [ ] 发布到 crates.io

### v0.4 · "类 claude 体验"（2–3 周）
- [ ] 流式 token 输出
- [ ] 斜杠命令体系（/help /clear /sessions /resume /model /cost /tools）
- [ ] `@file` / `@url` 上下文注入
- [ ] 工具调用实时状态行
- [ ] 项目级 `.cctraveler/` 配置
- [ ] Homebrew tap + npm 包装

### v0.5 · "TUI 与生态"（1 个月）
- [ ] ratatui 全屏 TUI（可选 `--tui` 启用）
- [ ] MCP server 集成（`cctraveler mcp add ...`）
- [ ] 多 Provider（OpenAI 兼容 / 国内厂商）
- [ ] 行程画布（边对话边可视化日程表）

---

## 7. 风险与开放问题

1. **Scraper 依赖**：CLI 价值依赖 scraper 在线。MVP 阶段是否提供官方托管服务？需评估反爬合规与成本。
2. **API key 分发**：是否提供"试用 key"？还是必须用户自带？
3. **流式 markdown 渲染**：边流边渲会导致代码块/表格半成品闪烁，需要"段落级 buffer + flush"策略。
4. **Windows 终端兼容**：rustyline / crossterm 在老 cmd.exe 上的多字节中文输入存在已知 bug，建议文档强调用 Windows Terminal。
5. **Session 体积**：长对话 + 工具结果可能上 MB，需考虑压缩或裁剪策略。

---

## 8. 验收标准（DoD for v0.3.1）

- [ ] `cargo install cctraveler` 后，新机器执行 `cctraveler` 能在 5 秒内进入对话
- [ ] 首次启动若无 config，引导补全后自动继续
- [ ] `cctraveler -p "上海到北京最便宜高铁"` 在 30 秒内返回结构化结果并退出
- [ ] `cctraveler doctor` 输出全部 ✔ 即代表环境就绪
- [ ] 老命令 `cctraveler scrape/search/export/chat` 行为完全不变

---

## 附：关键改动文件清单

```
crates/cli/src/main.rs         # 顶层 Cli 改造、默认子命令
crates/cli/src/repl/           # 新增：交互式 REPL（流式、斜杠命令、状态栏）
crates/cli/src/oneshot.rs      # 新增：-p 一次性模式
crates/cli/src/init.rs         # 新增：init 向导
crates/cli/src/doctor.rs       # 新增：环境诊断
crates/runtime/src/conversation.rs  # 新增 run_turn_streaming
crates/runtime/src/config.rs   # 配置层级合并（user / project / cli）
crates/runtime/src/session/    # 新增：session 列表与恢复
scripts/install.sh             # 新增：一行安装
.github/workflows/release.yml  # 新增：矩阵构建 + 发布
packages/cctraveler-npm/       # 可选：npm 包装
```
