# CCTraveler — Project Architecture

> AI Agent-powered hotel price intelligence platform.
> Scrapes Ctrip hotel data via stealth browser automation, orchestrated by a Rust agent harness.

---

## 1. Overview

CCTraveler is a **monorepo** project that combines:

1. **Agent Core** (Rust) — An AI agent harness inspired by [claw-code](https://github.com/colna/claw-code), responsible for task orchestration, tool execution, and intelligent scraping workflow management.
2. **Scraper Service** (Python) — A Scrapling-based stealth scraping microservice that handles Ctrip's anti-bot protections (TLS fingerprinting, Cloudflare bypass, browser automation).
3. **Web Frontend** (TypeScript/Next.js) — A dashboard to browse, search, and analyze scraped hotel data.

### Why This Architecture?

| Challenge | Solution |
|-----------|----------|
| Ctrip's heavy anti-bot (TLS fingerprinting, Cloudflare Turnstile, dynamic rendering) | Scrapling's `StealthyFetcher` with Patchright + canvas noise + WebRTC blocking |
| Login wall for price data | Browser session persistence + cookie management |
| Complex scraping workflows (pagination, retries, rate limiting) | Rust agent orchestrates tasks with tool-use pattern |
| Viewing scraped data | Next.js dashboard with search, filter, and price comparison |

---

## 2. Monorepo Structure

```
CCTraveler/
├── turbo.json                    # Turborepo pipeline config
├── package.json                  # Root workspace config
├── pnpm-workspace.yaml           # pnpm workspace definition
├── Cargo.toml                    # Rust workspace root
│
├── crates/                       # ═══ Rust Agent Core ═══
│   ├── agent-core/               # Core agent loop, conversation runtime
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs        # Agent conversation loop
│   │       ├── config.rs         # Config loading (TOML/JSON)
│   │       └── session.rs        # Session persistence
│   │
│   ├── tools/                    # Tool registry & execution
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # ToolSpec, GlobalToolRegistry
│   │       ├── scrape.rs         # Scrape tool — calls Python scraper
│   │       ├── search.rs         # Search tool — query scraped data
│   │       ├── analyze.rs        # Analyze tool — price comparison
│   │       └── export.rs         # Export tool — CSV/JSON output
│   │
│   ├── scraper-bridge/           # Rust ↔ Python scraper communication
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs         # HTTP client to scraper service
│   │       └── types.rs          # Shared data types (Hotel, Room, Price)
│   │
│   ├── storage/                  # Data persistence layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── db.rs             # SQLite storage
│   │       ├── models.rs         # Hotel, Room, PriceSnapshot
│   │       └── queries.rs        # Query builders
│   │
│   └── cli/                      # CLI binary entry point
│       ├── Cargo.toml
│       └── src/
│           └── main.rs           # CLI args, REPL, one-shot mode
│
├── services/                     # ═══ Python Scraper Service ═══
│   └── scraper/
│       ├── pyproject.toml        # Python project config (uv/pip)
│       ├── requirements.txt
│       ├── src/
│       │   ├── __init__.py
│       │   ├── server.py         # FastAPI HTTP service
│       │   ├── ctrip/
│       │   │   ├── __init__.py
│       │   │   ├── fetcher.py    # StealthyFetcher wrapper for Ctrip
│       │   │   ├── parser.py     # HTML → structured hotel data
│       │   │   ├── session.py    # Login session management
│       │   │   └── types.py      # Pydantic models
│       │   ├── anti_detect/
│       │   │   ├── __init__.py
│       │   │   ├── fingerprint.py  # TLS/browser fingerprint rotation
│       │   │   └── proxy.py      # Proxy pool management
│       │   └── utils/
│       │       ├── __init__.py
│       │       └── rate_limit.py # Request throttling
│       └── tests/
│           └── test_ctrip.py
│
├── packages/                     # ═══ Frontend & Shared ═══
│   ├── web/                      # Next.js dashboard
│   │   ├── package.json
│   │   ├── next.config.ts
│   │   ├── tailwind.config.ts
│   │   ├── app/
│   │   │   ├── layout.tsx
│   │   │   ├── page.tsx          # Home — search hotels
│   │   │   ├── hotels/
│   │   │   │   ├── page.tsx      # Hotel list with filters
│   │   │   │   └── [id]/
│   │   │   │       └── page.tsx  # Hotel detail + price history
│   │   │   └── api/
│   │   │       ├── hotels/
│   │   │       │   └── route.ts  # GET /api/hotels
│   │   │       ├── scrape/
│   │   │       │   └── route.ts  # POST /api/scrape (trigger)
│   │   │       └── prices/
│   │   │           └── route.ts  # GET /api/prices
│   │   └── components/
│   │       ├── hotel-card.tsx
│   │       ├── price-chart.tsx
│   │       ├── search-form.tsx
│   │       ├── filter-panel.tsx
│   │       └── data-table.tsx
│   │
│   └── shared/                   # Shared TypeScript types
│       ├── package.json
│       └── src/
│           └── types.ts          # Hotel, Room, Price types (TS)
│
├── docs/                         # ═══ Documentation ═══
│   ├── architecture.md           # This file
│   ├── scraping-strategy.md      # Ctrip anti-bot bypass details
│   └── api-reference.md          # Internal API docs
│
└── scripts/                      # ═══ Dev Scripts ═══
    ├── setup.sh                  # Install all dependencies
    └── dev.sh                    # Start all services
```

---

## 3. Component Architecture

### 3.1 Agent Core (Rust)

Inspired by claw-code's architecture, the agent core implements a **model-in-the-loop** pattern:

```
                    ┌──────────────────────────────┐
                    │         Agent Runtime         │
                    │                               │
User/Schedule ──────►  System Prompt + Context      │
                    │         │                     │
                    │         ▼                     │
                    │    LLM API Call               │
                    │    (Anthropic/OpenAI)          │
                    │         │                     │
                    │         ▼                     │
                    │    Response Stream             │
                    │    ├── Text (status updates)   │
                    │    └── ToolUse                 │
                    │         │                     │
                    │         ▼                     │
                    │    Tool Executor               │
                    │    ├── ScrapeHotels            │
                    │    ├── SearchData              │
                    │    ├── AnalyzePrices           │
                    │    └── ExportReport            │
                    │         │                     │
                    │         ▼                     │
                    │    Tool Result → next turn     │
                    │    (loop until done)           │
                    └──────────────────────────────┘
```

#### Key Crates

| Crate | Responsibility |
|-------|---------------|
| `agent-core` | Conversation loop, session management, config. Defines `AgentRuntime` trait |
| `tools` | Tool registry with `ToolSpec` definitions. Each tool has name, description, JSON Schema input, and execute fn |
| `scraper-bridge` | HTTP client that talks to the Python scraper service. Handles request/response serialization |
| `storage` | SQLite-backed persistence for hotels, rooms, price snapshots. Uses `rusqlite` |
| `cli` | Binary entry point. Supports REPL mode and one-shot commands |

#### Rust Workspace Dependencies

```toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
rusqlite = { version = "0.32", features = ["bundled"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

### 3.2 Scraper Service (Python)

A lightweight **FastAPI** microservice that wraps Scrapling for Ctrip-specific scraping:

```
              ┌─────────────────────────────────┐
              │       Scraper Service            │
              │       (FastAPI, port 8300)        │
              │                                   │
  HTTP ───────►  POST /scrape/hotels              │
              │    ├── city, checkin, checkout     │
              │    ├── filters (price, star, etc)  │
              │    │                               │
              │    ▼                               │
              │  CripFetcher                       │
              │    ├── StealthyFetcher             │
              │    │   ├── Patchright browser       │
              │    │   ├── TLS impersonation        │
              │    │   ├── Canvas noise              │
              │    │   ├── Cloudflare solver          │
              │    │   └── Proxy rotation             │
              │    │                               │
              │    ▼                               │
              │  CtripParser                       │
              │    ├── Extract hotel list            │
              │    ├── Parse room types + prices     │
              │    └── Handle pagination             │
              │                                   │
              │  Response: List[Hotel]              │
              └─────────────────────────────────┘
```

#### Ctrip Scraping Strategy

1. **Anti-detection**: Use `StealthyFetcher` with:
   - `hide_canvas=True` — defeat canvas fingerprinting
   - `block_webrtc=True` — prevent IP leak
   - `solve_cloudflare=True` — auto-solve Turnstile challenges
   - TLS fingerprint impersonation (`impersonate='chrome'`)

2. **Session management**:
   - Maintain persistent browser profiles with cookies
   - Rotate user-agent and fingerprints per session
   - Use proxy pool to distribute requests

3. **Data extraction**:
   - Ctrip renders hotel list via JavaScript (React SSR + client hydration)
   - Use browser automation to wait for dynamic content
   - Parse hotel cards: name, star rating, location, user rating, room types, prices
   - Handle infinite scroll / pagination via URL parameter `page=N`

4. **Rate limiting**:
   - Random delay between requests (2-5 seconds)
   - Max concurrent browser instances: 3
   - Backoff on 403/429 responses

#### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scrape/hotels` | Scrape hotel list for city + dates |
| `POST` | `/scrape/hotel/{id}` | Scrape single hotel detail page |
| `GET` | `/health` | Health check |
| `GET` | `/sessions` | List active browser sessions |

### 3.3 Web Frontend (Next.js)

A modern dashboard for viewing and analyzing scraped data:

#### Pages

| Route | Description |
|-------|-------------|
| `/` | Search form — select city, dates, filters |
| `/hotels` | Hotel list — cards with name, price, rating, photo |
| `/hotels/[id]` | Hotel detail — room types, price history chart, amenities |

#### Key Components

- **SearchForm** — City autocomplete, date picker, guest count, star filter
- **HotelCard** — Compact hotel preview with key info and lowest price
- **PriceChart** — Line chart showing price trends over time (recharts)
- **FilterPanel** — Price range slider, star rating, distance, amenities
- **DataTable** — Sortable, paginated table view of all scraped data

#### Data Flow

```
Frontend ──► Next.js API Routes ──► Rust Agent CLI (subprocess)
                                         │
                                         ├── SQLite (read scraped data)
                                         └── Scraper Service (trigger new scrapes)
```

---

## 4. Data Model

### Hotel

```typescript
interface Hotel {
  id: string;              // Ctrip hotel ID
  name: string;
  nameEn: string;
  star: number;            // 1-5
  rating: number;          // User rating (0-5.0)
  ratingCount: number;     // Number of reviews
  address: string;
  latitude: number;
  longitude: number;
  imageUrl: string;
  amenities: string[];
  city: string;
  district: string;
  createdAt: string;       // ISO 8601
  updatedAt: string;
}
```

### Room

```typescript
interface Room {
  id: string;
  hotelId: string;
  name: string;            // "大床房", "双床房", etc.
  bedType: string;
  maxGuests: number;
  area: number;            // sqm
  hasWindow: boolean;
  hasBreakfast: boolean;
  cancellationPolicy: string;
}
```

### PriceSnapshot

```typescript
interface PriceSnapshot {
  id: string;
  roomId: string;
  hotelId: string;
  price: number;           // CNY per night
  originalPrice: number;   // Before discount
  checkin: string;         // Date
  checkout: string;
  scrapedAt: string;       // When this price was captured
  source: string;          // "ctrip"
}
```

### SQLite Schema

```sql
CREATE TABLE hotels (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  name_en TEXT,
  star INTEGER,
  rating REAL,
  rating_count INTEGER,
  address TEXT,
  latitude REAL,
  longitude REAL,
  image_url TEXT,
  amenities TEXT,           -- JSON array
  city TEXT NOT NULL,
  district TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE rooms (
  id TEXT PRIMARY KEY,
  hotel_id TEXT NOT NULL REFERENCES hotels(id),
  name TEXT NOT NULL,
  bed_type TEXT,
  max_guests INTEGER,
  area REAL,
  has_window BOOLEAN,
  has_breakfast BOOLEAN,
  cancellation_policy TEXT
);

CREATE TABLE price_snapshots (
  id TEXT PRIMARY KEY,
  room_id TEXT NOT NULL REFERENCES rooms(id),
  hotel_id TEXT NOT NULL REFERENCES hotels(id),
  price REAL NOT NULL,
  original_price REAL,
  checkin TEXT NOT NULL,
  checkout TEXT NOT NULL,
  scraped_at TEXT NOT NULL,
  source TEXT DEFAULT 'ctrip'
);

CREATE INDEX idx_prices_hotel ON price_snapshots(hotel_id);
CREATE INDEX idx_prices_date ON price_snapshots(checkin, checkout);
CREATE INDEX idx_prices_scraped ON price_snapshots(scraped_at);
CREATE INDEX idx_hotels_city ON hotels(city);
```

---

## 5. Agent Tool Definitions

The Rust agent exposes these tools to the LLM:

### `scrape_hotels`

```json
{
  "name": "scrape_hotels",
  "description": "Scrape hotel listings from Ctrip for a given city and date range",
  "input_schema": {
    "type": "object",
    "properties": {
      "city": { "type": "string", "description": "City name or Ctrip city ID" },
      "checkin": { "type": "string", "description": "Check-in date (YYYY-MM-DD)" },
      "checkout": { "type": "string", "description": "Check-out date (YYYY-MM-DD)" },
      "max_pages": { "type": "integer", "description": "Max pages to scrape (default 5)" },
      "filters": {
        "type": "object",
        "properties": {
          "min_star": { "type": "integer" },
          "max_price": { "type": "number" },
          "keywords": { "type": "string" }
        }
      }
    },
    "required": ["city", "checkin", "checkout"]
  }
}
```

### `search_hotels`

```json
{
  "name": "search_hotels",
  "description": "Search previously scraped hotel data from local database",
  "input_schema": {
    "type": "object",
    "properties": {
      "city": { "type": "string" },
      "min_price": { "type": "number" },
      "max_price": { "type": "number" },
      "min_star": { "type": "integer" },
      "min_rating": { "type": "number" },
      "sort_by": { "type": "string", "enum": ["price", "rating", "star"] },
      "limit": { "type": "integer" }
    }
  }
}
```

### `analyze_prices`

```json
{
  "name": "analyze_prices",
  "description": "Analyze price trends and compare hotels",
  "input_schema": {
    "type": "object",
    "properties": {
      "hotel_ids": { "type": "array", "items": { "type": "string" } },
      "date_range": {
        "type": "object",
        "properties": {
          "start": { "type": "string" },
          "end": { "type": "string" }
        }
      },
      "comparison_type": { "type": "string", "enum": ["trend", "cheapest", "best_value"] }
    },
    "required": ["hotel_ids"]
  }
}
```

### `export_report`

```json
{
  "name": "export_report",
  "description": "Export scraped data as CSV or JSON file",
  "input_schema": {
    "type": "object",
    "properties": {
      "format": { "type": "string", "enum": ["csv", "json"] },
      "city": { "type": "string" },
      "checkin": { "type": "string" },
      "checkout": { "type": "string" }
    },
    "required": ["format"]
  }
}
```

---

## 6. Build & Dev Workflow

### Prerequisites

- Rust 1.80+ (with `cargo`)
- Python 3.10+ (with `uv` or `pip`)
- Node.js 20+ (with `pnpm`)
- Chromium (installed by Playwright/Patchright)

### Turborepo Pipeline

```json
// turbo.json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".next/**", "target/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "lint": {
      "dependsOn": ["^build"]
    },
    "test": {
      "dependsOn": ["build"]
    }
  }
}
```

### Commands

```bash
# Install all dependencies
pnpm install                      # Node dependencies
cd crates && cargo build          # Rust workspace
cd services/scraper && uv sync    # Python dependencies

# Development (starts all services)
pnpm dev
# Equivalent to:
#   - cargo run -p cli              (agent CLI)
#   - python services/scraper/src/server.py  (scraper on :8300)
#   - next dev packages/web         (frontend on :3000)

# Build
pnpm build

# Lint
pnpm lint                         # ESLint (TS)
cargo clippy --workspace          # Clippy (Rust)
ruff check services/scraper       # Ruff (Python)

# Test
cargo test --workspace            # Rust tests
pytest services/scraper/tests     # Python tests
```

---

## 7. Configuration

### `config.toml` (Agent)

```toml
[agent]
model = "claude-sonnet-4-20250514"
max_turns = 50

[scraper]
base_url = "http://localhost:8300"
timeout_secs = 120
max_retries = 3

[storage]
db_path = "data/cctraveler.db"

[ctrip]
default_city = "558"          # Zunyi
default_adults = 1
default_children = 0
request_delay_ms = 3000
max_concurrent = 3
proxy_pool = []               # Optional proxy list
```

---

## 8. Deployment

### Local Development

All services run locally. The Rust CLI acts as the orchestrator, the Python scraper runs as a sidecar, and the frontend connects to both.

### Production (Future)

```
┌──────────┐     ┌──────────────┐     ┌──────────┐
│ Frontend │────►│  Rust Agent   │────►│ Scraper  │
│ (Vercel) │     │  (VPS/Docker) │     │ (Docker) │
└──────────┘     └──────┬───────┘     └──────────┘
                        │
                   ┌────▼────┐
                   │ SQLite  │
                   │  / PG   │
                   └─────────┘
```

---

## 9. Tech Stack Summary

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Agent Runtime | Rust (tokio, reqwest, clap) | Task orchestration, tool execution |
| Scraper | Python (Scrapling, FastAPI) | Stealth web scraping with anti-bot bypass |
| Storage | SQLite (rusqlite) | Hotel & price data persistence |
| Frontend | Next.js 15, Tailwind CSS | Dashboard UI |
| Monorepo | Turborepo + pnpm + Cargo workspace | Build orchestration |
| Charts | Recharts | Price trend visualization |

---

## 10. Roadmap

### Phase 1 — MVP
- [ ] Project scaffolding (monorepo, configs)
- [ ] Python scraper service with Ctrip hotel list parsing
- [ ] Rust storage layer (SQLite)
- [ ] Basic CLI to trigger scrapes and query data
- [ ] Minimal frontend with hotel list view

### Phase 2 — Agent Intelligence
- [ ] Full agent loop with LLM integration
- [ ] Tool definitions (scrape, search, analyze, export)
- [ ] Price comparison and trend analysis
- [ ] Session management for multi-step workflows

### Phase 3 — Production Hardening
- [ ] Proxy pool management
- [ ] Scheduled scraping (cron-like)
- [ ] Price alert notifications
- [ ] Multi-city support
- [ ] Hotel detail page scraping (room-level data)

### Phase 4 — Advanced Features
- [ ] Price prediction (ML)
- [ ] Multi-source comparison (Ctrip + Meituan + Fliggy)
- [ ] Mobile-responsive dashboard
- [ ] Export to popular travel planning tools
