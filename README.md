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
│  storage     │    JSON      │  Scrapling       │
│  tools       │              └─────────────────┘
│  runtime     │
└──────┬───────┘
       │ SQLite
       ▼
┌─────────────┐
│  Next.js     │
│  (packages/) │
│  Web :3000   │
│  Vercel      │
└─────────────┘
```

| Layer | Tech | Path |
|-------|------|------|
| **Agent Core** | Rust (tokio, clap, rusqlite) | `crates/` |
| **Scraper** | Python (FastAPI, Scrapling) | `services/scraper/` |
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

### Quick Start

```bash
# 1. Clone
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. Install all dependencies
./scripts/setup.sh

# 3. Start dev servers (scraper + frontend)
./scripts/dev.sh
```

Or manually:

```bash
# Build Rust
cargo build --workspace

# Start Python scraper
cd services/scraper
python -m uvicorn src.server:app --port 8300

# Start Next.js frontend
cd packages/web
pnpm dev
```

### CLI Usage

```bash
# Scrape hotels in Shanghai, checking in May 1 → May 3
cargo run -p cli -- scrape --city shanghai --checkin 2026-05-01 --checkout 2026-05-03

# Search stored hotels
cargo run -p cli -- search --city shanghai --max-price 800 --min-star 4

# Export data
cargo run -p cli -- export --format csv --output hotels.csv
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
│   └── scraper/      # Python FastAPI + Scrapling
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
pnpm --filter web build    # builds to packages/web/.next
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
| **爬虫服务** | Python (FastAPI, Scrapling) | `services/scraper/` |
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

### 快速开始

```bash
# 1. 克隆仓库
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. 一键安装所有依赖
./scripts/setup.sh

# 3. 启动开发服务（爬虫 + 前端）
./scripts/dev.sh
```

### CLI 使用

```bash
# 爬取上海酒店，入住 5/1 → 退房 5/3
cargo run -p cli -- scrape --city shanghai --checkin 2026-05-01 --checkout 2026-05-03

# 搜索已存储的酒店
cargo run -p cli -- search --city shanghai --max-price 800 --min-star 4

# 导出数据
cargo run -p cli -- export --format csv --output hotels.csv
```

### 数据流

```
用户 CLI 命令
    ↓
Rust CLI (crates/cli)
    ↓ HTTP POST
Python 爬虫 (FastAPI :8300)
    ↓ Scrapling + 反检测
携程网页
    ↓ JSON 解析
Rust 存入 SQLite
    ↓
Next.js 前端读取展示 (:3000)
```

### 部署

**前端** → Vercel：
```bash
pnpm --filter web build
```

已配置 `vercel.json`，在 Vercel 连接仓库即可自动部署。

### 文档

- [架构设计（中文）](docs/architecture-zh.md)
- [架构设计（英文）](docs/architecture.md)
- [产品设计文档](docs/product.md)

### 许可

MIT
