# CCTraveler

> **AI Travel Planner** — Plan trips, compare routes, track prices, get optimal itineraries.

[中文版](README-zh.md)

---

### What is CCTraveler?

CCTraveler is an **AI-powered full-chain travel planning platform**. It goes beyond hotel search — like a personal travel advisor, it combines hotels, flights, trains, and city geography to automatically plan complete itineraries and recommend optimal routes.

**Current MVP**: Hotel data scraping and price comparison from Trip.com / Ctrip, with an AI agent chat interface.

**Roadmap**: Flights → Trains → City geography → AI-generated itineraries.

### Architecture

```
┌─────────────┐     HTTP      ┌─────────────────┐
│  Rust CLI    │ ──────────── │  Python Scraper  │
│  (crates/)   │    POST      │  (services/)     │
│              │ ◄────────── │  FastAPI :8300    │
│  storage     │    JSON      │  httpx           │
│  tools       │              └─────────────────┘
│  runtime     │                      │
│  api         │              Trip.com / Ctrip
└──────┬───────┘              (RSC payload with prices)
       │ SQLite
       ▼
┌─────────────┐
│  Next.js     │
│  (packages/) │
│  Web :3100   │
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
| `runtime` | Config loading (TOML), conversation runtime, session management |
| `api` | Anthropic API client with SSE streaming |
| `tools` | Scrape, search, analyze, export tool implementations |
| `cli` | Binary entry point with `scrape`, `search`, `export`, `chat` subcommands |

### Prerequisites

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Node.js** (20+) + **pnpm** (9+) — `npm i -g pnpm`
- **Python** (3.10+)

### Quick Start

```bash
# 1. Clone
git clone git@github.com:colna/CCTraveler.git
cd CCTraveler

# 2. Configure API key
cp config.toml.example config.toml
# Edit config.toml — set api_key and base_url

# 3. Install all dependencies
pnpm setup
# or: ./scripts/setup.sh

# 4. Start all dev servers (scraper + frontend)
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

# AI Agent Chat (requires API key in config.toml)
cargo run -p cctraveler -- chat
```

Or with `cargo` directly:

```bash
cargo run -p cctraveler -- scrape --city shanghai --checkin 2026-05-01 --checkout 2026-05-02
cargo run -p cctraveler -- search --city shanghai --max-price 800 --min-star 4
cargo run -p cctraveler -- export --format csv --output hotels.csv
cargo run -p cctraveler -- chat
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
Trip.com (RSC payload with prices)
         │  hotelList[] JSON extraction
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
│   ├── api/          # Anthropic API client (SSE streaming)
│   ├── cli/          # CLI binary (clap) + chat REPL
│   ├── runtime/      # Config, conversation runtime, session, prompt builder
│   ├── storage/      # SQLite persistence
│   └── tools/        # Scrape/search/analyze/export tools
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
├── config.toml.example  # Configuration template
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
