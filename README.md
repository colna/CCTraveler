# CCTraveler

> **AI Travel Planner** — Plan trips, compare routes, track prices, get optimal itineraries.
>
> **AI 出行规划师** — 规划行程、比选路线、追踪价格，给你最优出行方案。

[English](#english) | [中文](#中文)

---

## English

### What is CCTraveler?

CCTraveler is an **AI-powered full-chain travel planning platform**. It goes beyond hotel search — like a personal travel advisor, it combines hotels, flights, trains, and city geography to automatically plan complete itineraries and recommend optimal routes.

**Current MVP**: Hotel data scraping and price comparison from Ctrip.

**Roadmap**: Flights → Trains → City geography → AI-generated itineraries.

### Architecture

```
┌─────────────┐     HTTP      ┌─────────────────┐
│  Rust CLI    │ ──────────── │  Python Scraper  │
│  (crates/)   │    POST      │  (services/)     │
│              │ ◄────────── │  FastAPI :8300    │
│  storage     │    JSON      │  httpx           │
│  tools       │              └─────────────────┘
│  runtime     │
└──────┬───────┘
       │ SQLite
       ▼
┌─────────────┐
│  Next.js     │
│  (packages/) │
│  Web :3100   │
│  Vercel      │
└─────────────┘
```

| Layer | Tech | Path |
|-------|------|------|
| **Agent Core** | Rust (tokio, clap, rusqlite) | `crates/` |
| **Scraper** | Python (FastAPI, httpx) | `services/scraper/` |
| **Frontend** | Next.js 15, React 19, Tailwind v4 | `packages/web/` |
| **Shared Types** | TypeScript | `packages/shared/` |
| **Build** | Turborepo + pnpm + Cargo workspace | root |

### Rust Crates

| Crate | Description |
|-------|-------------|
| `storage` | SQLite database — hotels, rooms, price snapshots |
| `runtime` | Config loading (TOML), conversation types |
| `api` | `ApiClient` trait (stub, full LLM integration in P1) |
| `tools` | Scrape, search, export tool implementations |
| `cli` | Binary entry point with `scrape`, `search`, `export` subcommands |

### Prerequisites

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Node.js** (20+) + **pnpm** (9+) — `npm i -g pnpm`
- **Python** (3.10+)

### Quick Start

```bash
# 1. Clone
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. Install all dependencies
pnpm setup
# or: ./scripts/setup.sh

# 3. Start all dev servers (scraper + frontend)
pnpm dev:all
```

Or start services separately:

```bash
# Terminal 1 — Python scraper (:8300)
pnpm dev:scraper

# Terminal 2 — Next.js frontend (:3100)
pnpm dev:web

# Build Rust workspace
pnpm build:rust
```

### CLI Usage

```bash
# Scrape hotels in Shanghai, May 1 → May 2
pnpm scrape -- --city shanghai --checkin 2026-05-01 --checkout 2026-05-02

# Scrape Beijing, up to 3 pages
pnpm scrape -- --city beijing --checkin 2026-05-01 --checkout 2026-05-02 --max-pages 3

# Search stored hotels — 4-star+, ¥800 max
pnpm search -- --city shanghai --max-price 800 --min-star 4

# Export to CSV
pnpm export -- --format csv --output hotels.csv

# Export to JSON
pnpm export -- --format json --output hotels.json
```

Or with `cargo` directly:

```bash
cargo run -p cli -- scrape --city shanghai --checkin 2026-05-01 --checkout 2026-05-02
cargo run -p cli -- search --city shanghai --max-price 800 --min-star 4
cargo run -p cli -- export --format csv --output hotels.csv
```

### Supported Cities

| English | Chinese | City ID |
|---------|---------|---------|
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

### npm Scripts

| Command | Description |
|---------|-------------|
| `pnpm dev:all` | Start all services (scraper + frontend) |
| `pnpm dev:scraper` | Start scraper only (:8300) |
| `pnpm dev:web` | Start frontend only (:3100) |
| `pnpm build:rust` | Build Rust workspace (debug) |
| `pnpm build:rust:release` | Build Rust workspace (release) |
| `pnpm build:web` | Build Next.js for production |
| `pnpm lint:rust` | Run clippy on Rust workspace |
| `pnpm test:rust` | Run Rust tests |
| `pnpm setup` | Install all dependencies |
| `pnpm setup:python` | Setup Python venv only |
| `pnpm scrape` | CLI scrape subcommand |
| `pnpm search` | CLI search subcommand |
| `pnpm export` | CLI export subcommand |

### Data Flow

```
CLI: pnpm scrape -- --city shanghai ...
         │
         ▼  HTTP POST
Python Scraper (FastAPI :8300)
         │  httpx + browser-like headers
         ▼
Ctrip Website
         │  __INITIAL_STATE__ JSON extraction
         ▼
Rust stores → SQLite (data/cctraveler.db)
         │
         ▼
Next.js Frontend (:3100/hotels)
```

### Project Structure

```
CCTraveler/
├── crates/
│   ├── api/          # API client trait
│   ├── cli/          # CLI binary (clap)
│   ├── runtime/      # Config & types
│   ├── storage/      # SQLite persistence
│   └── tools/        # Scrape/search/export
├── services/
│   └── scraper/      # Python FastAPI + httpx
├── packages/
│   ├── web/          # Next.js frontend
│   └── shared/       # Shared TypeScript types
├── scripts/
│   ├── setup.sh      # One-click install
│   └── dev.sh        # Start all services
├── docs/
│   ├── architecture.md
│   ├── architecture-zh.md
│   └── product.md
├── config.toml       # Runtime configuration
├── vercel.json       # Vercel deployment
├── Cargo.toml        # Rust workspace
├── package.json      # pnpm + Turborepo
└── turbo.json        # Build pipeline
```

### Deployment

**Frontend** → Vercel:
```bash
pnpm build:web    # builds to packages/web/.next
```

`vercel.json` is pre-configured. Connect the repo on Vercel and it deploys automatically.

### Docs

- [Architecture (EN)](docs/architecture.md)
- [Architecture (ZH)](docs/architecture-zh.md)
- [Product Design](docs/product.md)

### License

MIT

---

## 中文

### CCTraveler 是什么？

CCTraveler 是一个 **AI 驱动的全链路出行规划平台**。它不只是查酒店——而是像你的私人旅行顾问，综合酒店、机票、火车、城市地理信息，自动规划完整行程，给出最优路线和方案。

**当前 MVP**：携程酒店数据爬取与价格比较。

**路线图**：机票 → 火车票 → 城市地理 → AI 智能行程规划。

### 技术架构

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
| `runtime` | 配置加载 (TOML)、对话类型定义 |
| `api` | `ApiClient` trait（MVP 阶段为 stub，P1 接入 LLM） |
| `tools` | 爬取、搜索、导出工具实现 |
| `cli` | 二进制入口，含 `scrape`/`search`/`export` 子命令 |

### 环境要求

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Node.js** (20+) + **pnpm** (9+) — `npm i -g pnpm`
- **Python** (3.10+)

### 快速开始

```bash
# 1. 克隆仓库
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. 一键安装所有依赖
pnpm setup

# 3. 启动全部开发服务（爬虫 + 前端）
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

# 导出 JSON
pnpm export -- --format json --output hotels.json
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
| `pnpm setup:python` | 仅安装 Python 环境 |
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
携程网页
         │  提取 __INITIAL_STATE__ JSON
         ▼
Rust 存入 SQLite (data/cctraveler.db)
         │
         ▼
Next.js 前端展示 (:3100/hotels)
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
