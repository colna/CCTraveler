# CCTraveler

> **AI 出行规划师** — 规划行程、比选路线、追踪价格，给你最优出行方案。

[English](README.md)

---

### CCTraveler 是什么？

CCTraveler 是一个 **AI 驱动的全链路出行规划平台**。它不只是查酒店——而是像你的私人旅行顾问，综合酒店、机票、火车、城市地理信息，自动规划完整行程，给出最优路线和方案。

**当前 MVP**：Trip.com / 携程酒店数据爬取与价格比较，含 AI Agent 对话界面。

**路线图**：机票 → 火车票 → 城市地理 → AI 智能行程规划。

### 技术架构

```
┌─────────────┐     HTTP      ┌─────────────────┐
│  Rust CLI    │ ──────────── │  Python 爬虫     │
│  (crates/)   │    POST      │  (services/)     │
│              │ ◄────────── │  FastAPI :8300    │
│  storage     │    JSON      │  httpx           │
│  tools       │              └─────────────────┘
│  runtime     │                      │
│  api         │              Trip.com / 携程
└──────┬───────┘              (RSC 数据含价格)
       │ SQLite
       ▼
┌─────────────┐
│  Next.js     │
│  (packages/) │
│  Web :3100   │
└─────────────┘
```

| 层级 | 技术栈 | 路径 |
|------|--------|------|
| **Agent 核心** | Rust (tokio, clap, rusqlite) | `crates/` |
| **爬虫服务** | Python (FastAPI, httpx) | `services/scraper/` |
| **前端** | Next.js 15, React 19, Tailwind v4 | `packages/web/` |
| **共享类型** | TypeScript | `packages/shared/` |
| **构建** | Turborepo + pnpm + Cargo workspace | 根目录 |

### Rust Crate 说明

| Crate | 功能 |
|-------|------|
| `storage` | SQLite 数据库 — 酒店、房型、价格快照 |
| `runtime` | 配置加载 (TOML)、对话运行时、会话管理 |
| `api` | Anthropic API 客户端（SSE 流式传输） |
| `tools` | 爬取、搜索、分析、导出工具实现 |
| `cli` | 二进制入口，含 `scrape`/`search`/`export`/`chat` 子命令 |

### 环境要求

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Node.js** (20+) + **pnpm** (9+) — `npm i -g pnpm`
- **Python** (3.10+)

### 快速开始

```bash
# 1. 克隆仓库
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. 配置 API Key
cp config.toml.example config.toml
# 编辑 config.toml — 填写 api_key 和 base_url

# 3. 一键安装所有依赖
pnpm setup

# 4. 启动全部开发服务（爬虫 + 前端）
pnpm dev:all
```

或分别启动：

```bash
# 终端 1 — Python 爬虫 (:8300)
pnpm dev:scraper

# 终端 2 — Next.js 前端 (:3100)
pnpm dev:web

# 编译 Rust
pnpm build:rust
```

### CLI 使用

```bash
# 爬取上海酒店，5/1 入住 → 5/2 退房
pnpm scrape -- --city shanghai --checkin 2026-05-01 --checkout 2026-05-02

# 爬取北京，最多3页
pnpm scrape -- --city beijing --checkin 2026-05-01 --checkout 2026-05-02 --max-pages 3

# 搜索：四星以上、800元以下
pnpm search -- --city shanghai --max-price 800 --min-star 4

# 导出 CSV
pnpm export -- --format csv --output hotels.csv

# AI Agent 对话（需要在 config.toml 中配置 API key）
cargo run -p cctraveler -- chat
```

### 支持的城市

| 英文 | 中文 | 城市 ID |
|------|------|---------|
| shanghai | 上海 | 2 |
| beijing | 北京 | 1 |
| chengdu | 成都 | 28 |
| chongqing | 重庆 | 4 |
| hangzhou | 杭州 | 14 |
| shenzhen | 深圳 | 26 |
| guangzhou | 广州 | 32 |
| nanjing | 南京 | 9 |
| xian | 西安 | 7 |
| kunming | 昆明 | 31 |
| dali | 大理 | 135 |
| sanya | 三亚 | 43 |
| guiyang | 贵阳 | 30 |
| zunyi | 遵义 | 558 |

### npm 脚本速查

| 命令 | 说明 |
|------|------|
| `pnpm dev:all` | 启动所有服务（爬虫 + 前端） |
| `pnpm dev:scraper` | 单独启动爬虫 (:8300) |
| `pnpm dev:web` | 单独启动前端 (:3100) |
| `pnpm build:rust` | 编译 Rust (debug) |
| `pnpm build:rust:release` | 编译 Rust (release) |
| `pnpm build:web` | 构建前端 (production) |
| `pnpm lint:rust` | Clippy 检查 |
| `pnpm test:rust` | 运行 Rust 测试 |
| `pnpm setup` | 一键安装所有依赖 |
| `pnpm scrape` | CLI 爬取命令 |
| `pnpm search` | CLI 搜索命令 |
| `pnpm export` | CLI 导出命令 |

### 数据流

```
CLI: pnpm scrape -- --city shanghai ...
         │
         ▼  HTTP POST
Python 爬虫 (FastAPI :8300)
         │  httpx + 浏览器 UA 模拟
         ▼
Trip.com (RSC 数据含价格)
         │  提取 hotelList[] JSON
         ▼
Rust 存入 SQLite (data/cctraveler.db)
         │
         ▼
Next.js 前端展示 (:3100/hotels)
```

### 项目结构

```
CCTraveler/
├── crates/
│   ├── api/          # Anthropic API 客户端（SSE 流式）
│   ├── cli/          # CLI 入口（clap）+ chat 交互
│   ├── runtime/      # 配置、对话运行时、会话、提示词构建
│   ├── storage/      # SQLite 持久化
│   └── tools/        # 爬取/搜索/分析/导出工具
├── services/
│   └── scraper/      # Python FastAPI + httpx
├── packages/
│   ├── web/          # Next.js 前端
│   └── shared/       # 共享 TypeScript 类型
├── scripts/
│   ├── setup.sh      # 一键安装脚本
│   └── dev.sh        # 启动所有服务
├── docs/
│   ├── architecture.md
│   ├── architecture-zh.md
│   └── product.md
├── config.toml.example  # 配置模板
├── Cargo.toml        # Rust 工作区
├── package.json      # pnpm + Turborepo
└── turbo.json        # 构建流水线
```

### 部署

**前端** → Vercel：
```bash
pnpm build:web
```

已配置 `vercel.json`，在 Vercel 连接仓库即可自动部署。

### 文档

- [架构设计（中文）](docs/architecture-zh.md)
- [架构设计（英文）](docs/architecture.md)
- [产品设计文档](docs/product.md)

### 许可

MIT
