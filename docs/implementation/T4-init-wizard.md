# T4 · `cctraveler init` 配置向导

> 交互式向导，零配置生成 `~/.cctraveler/config.toml`，让新用户一行命令上手。

## 完成状态
✅ 已实现，编译通过。

## 思路与逻辑

### 痛点
新用户首次安装后必须手动：
1. 复制 `config.toml.example` → `config.toml`；
2. 知道去 `[agent]` section 填 `api_key`；
3. 配 `db_path` / `scraper.base_url` 等字段。
对终端 CLI 这是劝退体验。需要一个"问几个问题就出文件"的向导。

### 实现策略
- **复用 rustyline**：CLI 已经依赖 `rustyline`，直接拿 `DefaultEditor` 做 prompt，不引入 `dialoguer` 依赖（少一个 crate）。
- **每个问题都有合理默认值**：直接回车即采用 `[默认]`，最快路径只按 5 次回车即可完成。
- **覆写保护**：检测到 `~/.cctraveler/config.toml` 已存在时再次确认，避免误操作。
- **API key 留空合法**：用户也可以选择只用 `ANTHROPIC_API_KEY` 环境变量，向导识别并打提示。
- **模板字符串占位替换**：用 `{MODEL}` `{API_KEY}` 等占位符做最简模板，不引入 handlebars/tera。

### 默认值
| 字段 | 默认 | 说明 |
|---|---|---|
| `agent.model` | `claude-sonnet-4-20250514` | 性价比最高的当前默认 |
| `agent.api_key` | 空 | 鼓励走环境变量 |
| `scraper.base_url` | `http://localhost:8300` | 本地起 scraper 服务 |
| `storage.db_path` | `~/.cctraveler/data/cctraveler.db` | 全局共享数据库（与项目无关） |

### 与 T3 的协作
向导写入 `runtime::config::user_config_path()` 返回的路径（即 `~/.cctraveler/config.toml`），正好被 T3 的 layered loader 作为最低优先级源拾取。无需任何额外胶水。

## 改动文件
- `crates/cli/src/init.rs`（新增）
- `crates/cli/src/main.rs` — 路由 `Commands::Init → init::run()`

## 用法
```
$ cctraveler init
╭─ CCTraveler 初始化向导 ────────────────────╮
│ 我会帮你创建 ~/.cctraveler/config.toml      │
╰────────────────────────────────────────────╯

? Anthropic API key (留空则使用 ANTHROPIC_API_KEY 环境变量)
› sk-ant-...
? 默认模型 [claude-sonnet-4-20250514]
›
? Scraper 服务地址 [http://localhost:8300]
›
? SQLite 数据库路径 [/Users/me/.cctraveler/data/cctraveler.db]
›

✔ 已写入 /Users/me/.cctraveler/config.toml
现在可以直接运行 `cctraveler` 进入对话。
```

## 后续
- 试用版 API key 申请引导（v0.4，需后端配合）；
- 模型用 selection 而非自由输入（v0.4，需 `dialoguer::Select`）。
