# CCTraveler — Project Architecture

> AI Agent-powered hotel price intelligence platform.
> Scrapes Ctrip hotel data via stealth browser automation, orchestrated by a Rust agent harness.

---

## 1. Overview

CCTraveler is a **monorepo** project that combines:

1. **Agent Core** (Rust) — An AI agent harness modeled after [ultraworkers/claw-code](https://github.com/ultraworkers/claw-code), implementing the same `ConversationRuntime<C: ApiClient, T: ToolExecutor>` pattern for task orchestration with tool-use agent loops.
2. **Scraper Service** (Python) — A [Scrapling](https://github.com/D4Vinci/Scrapling)-based stealth scraping microservice that handles Ctrip's anti-bot protections (TLS fingerprinting, Cloudflare bypass, browser automation).
3. **Web Frontend** (TypeScript/Next.js) — A dashboard to browse, search, and analyze scraped hotel data.

### Reference Architecture: claw-code

Our Rust agent core adopts the following patterns from `ultraworkers/claw-code`:

| claw-code Pattern | CCTraveler Adoption |
|-------------------|---------------------|
| `ConversationRuntime<C: ApiClient, T: ToolExecutor>` — generic agent loop | Same pattern: generic runtime parameterized over API client + tool executor traits |
| `ToolSpec` + `GlobalToolRegistry` + match-dispatch | Same pattern: 4 domain tools (scrape, search, analyze, export) registered via `ToolSpec` |
| `Session` JSONL persistence with rotation | Simplified: single JSONL session file per task |
| `ConfigLoader` 3-layer merge (User > Project > Local) | Adapted: TOML-based config with workspace-level defaults |
| `SystemPromptBuilder` with instruction file discovery | Adapted: static system prompt tailored for hotel scraping domain |
| `PermissionPolicy` + `PermissionPrompter` trait | Simplified: no permission prompting (all tools pre-authorized) |
| Cargo workspace with 9 crates | Scaled down: 5 crates focused on scraping domain |

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
├── package.json                  # Root workspace config (pnpm)
├── pnpm-workspace.yaml           # pnpm workspace definition
├── Cargo.toml                    # Rust workspace root
├── Cargo.lock
│
├── crates/                       # ═══ Rust Agent Core ═══
│   ├── runtime/                  # Core: conversation loop, session, config, prompt
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Public re-exports
│   │       ├── conversation.rs   # ConversationRuntime<C,T> — the agent loop
│   │       ├── session.rs        # Session struct, JSONL persistence
│   │       ├── config.rs         # ConfigLoader, RuntimeConfig (TOML)
│   │       ├── prompt.rs         # SystemPromptBuilder for scraping domain
│   │       └── types.rs          # ConversationMessage, ContentBlock, MessageRole
│   │
│   ├── api/                      # LLM provider abstraction
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs         # ProviderClient enum (Anthropic/OpenAI-compat)
│   │       ├── types.rs          # MessageRequest, MessageResponse, ToolDefinition
│   │       ├── sse.rs            # SSE frame parser for streaming
│   │       └── providers/
│   │           ├── mod.rs        # Provider trait, ProviderKind enum
│   │           ├── anthropic.rs  # AnthropicClient (AuthSource, streaming, retries)
│   │           └── openai_compat.rs  # OpenAI/xAI compatible client
│   │
│   ├── tools/                    # Tool definitions and dispatch
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # ToolSpec, GlobalToolRegistry, execute_tool()
│   │       ├── scrape.rs         # scrape_hotels — calls Python scraper service
│   │       ├── search.rs         # search_hotels — query local SQLite DB
│   │       ├── analyze.rs        # analyze_prices — price comparison logic
│   │       └── export.rs         # export_report — CSV/JSON output
│   │
│   ├── storage/                  # Data persistence layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── db.rs             # SQLite connection pool
│   │       ├── models.rs         # Hotel, Room, PriceSnapshot (Rust structs)
│   │       └── queries.rs        # Query builders (insert, search, aggregate)
│   │
│   └── cli/                      # CLI binary entry point
│       ├── Cargo.toml
│       ├── build.rs              # Injects GIT_SHA, BUILD_DATE
│       └── src/
│           ├── main.rs           # main(), CLI arg parsing, REPL
│           ├── render.rs         # Terminal output, spinner, markdown
│           └── input.rs          # LineEditor (rustyline wrapper)
│
├── services/                     # ═══ Python Scraper Service ═══
│   └── scraper/
│       ├── pyproject.toml        # Python project config (uv/pip)
│       ├── requirements.txt
│       ├── src/
│       │   ├── __init__.py
│       │   ├── server.py         # FastAPI HTTP service (port 8300)
│       │   ├── ctrip/
│       │   │   ├── __init__.py
│       │   │   ├── fetcher.py    # StealthyFetcher wrapper for Ctrip
│       │   │   ├── parser.py     # HTML → structured hotel data
│       │   │   ├── session.py    # Browser session persistence + cookies
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

## 3. Architecture Diagrams

### 3.0 System Architecture

```mermaid
graph TB
    subgraph Frontend["Frontend (Next.js)"]
        UI["Search / List / Detail Pages"]
        API["API Routes"]
    end

    subgraph AgentCore["Agent Core (Rust)"]
        CLI["CLI / REPL"]
        RT["ConversationRuntime"]
        Tools["Tool Registry"]
        Storage["SQLite Storage"]
    end

    subgraph Scraper["Scraper Service (Python)"]
        FastAPI["FastAPI Server :8300"]
        Fetcher["StealthyFetcher"]
        Parser["CtripParser"]
    end

    subgraph External["External Services"]
        LLM["Anthropic API"]
        Ctrip["hotels.ctrip.com"]
    end

    UI --> API
    API --> CLI
    CLI --> RT
    RT -->|"stream"| LLM
    LLM -->|"tool_use"| RT
    RT --> Tools
    Tools -->|"scrape_hotels"| FastAPI
    Tools -->|"search/analyze"| Storage
    FastAPI --> Fetcher
    Fetcher -->|"anti-bot bypass"| Ctrip
    Ctrip --> Parser
    Parser --> FastAPI
    FastAPI --> Tools
    Tools --> RT
```

## 4. Core Rust Architecture (from claw-code)

### 4.1 Key Traits

Following claw-code's trait-based polymorphism pattern for testability:

```rust
/// API client trait — abstracts over LLM providers (Anthropic, OpenAI, etc.)
/// Synchronous interface; async is handled internally via tokio::Runtime.
pub trait ApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError>;
}

/// Tool executor trait — abstracts over tool dispatch
pub trait ToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError>;
}
```

Production implementations:
- `AnthropicRuntimeClient` implements `ApiClient` — wraps `ProviderClient` enum + tokio runtime
- `TravelerToolExecutor` implements `ToolExecutor` — dispatches to scrape/search/analyze/export handlers

Test implementations:
- `MockApiClient` — returns scripted responses for deterministic testing
- `MockToolExecutor` — records calls and returns preset results

### 4.2 The Agent Conversation Loop

`ConversationRuntime<C: ApiClient, T: ToolExecutor>` — the core agent loop, directly adapted from claw-code's `conversation.rs`:

```mermaid
flowchart TD
    A["User Input"] --> B["Push to Session"]
    B --> C["Build ApiRequest"]
    C --> D["Call LLM API stream"]
    D --> E{"Parse Response"}
    E -->|"Text"| F["Output Result"]
    E -->|"ToolUse"| G["Extract Tool Calls"]
    G --> H{"Dispatch Tool"}
    H -->|"scrape_hotels"| I["Call Python Scraper"]
    H -->|"search_hotels"| J["Query SQLite"]
    H -->|"analyze_prices"| K["Price Analysis"]
    H -->|"export_report"| L["Export CSV/JSON"]
    I --> M["Build ToolResult"]
    J --> M
    K --> M
    L --> M
    M --> N["Push to Session"]
    N --> C
    F --> O["Return TurnSummary"]
```

### 4.3 Core Types

```rust
/// A conversation message (user, assistant, or tool result)
pub struct ConversationMessage {
    pub role: MessageRole,           // System | User | Assistant | Tool
    pub content: Vec<ContentBlock>,
    pub usage: Option<TokenUsage>,
}

/// Content within a message
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, tool_name: String, output: String, is_error: bool },
}

/// Session state persisted as JSONL
pub struct Session {
    pub session_id: String,
    pub messages: Vec<ConversationMessage>,
    pub workspace_root: PathBuf,
    pub model: String,
    pub created_at: DateTime<Utc>,
}

/// Tool definition exposed to the LLM
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,  // JSON Schema
}

/// Merged tool registry
pub struct GlobalToolRegistry {
    tools: Vec<ToolSpec>,
}
```

### 4.4 Crate Dependency Graph

```mermaid
graph BT
    runtime["runtime\nCore Engine"]
    api["api\nLLM Providers"]
    tools["tools\nTool Registry"]
    storage["storage\nData Persistence"]
    cli["cli\nBinary Entry"]

    api --> runtime
    tools --> api
    tools --> runtime
    tools --> storage
    storage --> runtime
    cli --> api
    cli --> tools
    cli --> storage
```

| Crate | Responsibility |
|-------|---------------|
| `runtime` | Core engine: `ConversationRuntime<C,T>`, session persistence, config loading, system prompt builder, core types (`ConversationMessage`, `ContentBlock`) |
| `api` | LLM provider abstraction: `ProviderClient` enum, SSE streaming, Anthropic + OpenAI-compat clients, retry logic |
| `tools` | Tool inventory: `ToolSpec` definitions, `GlobalToolRegistry`, `execute_tool()` match dispatch to typed handlers |
| `storage` | Data layer: SQLite via `rusqlite`, Hotel/Room/PriceSnapshot models, query builders |
| `cli` | Binary entry point: CLI arg parsing via `clap`, REPL mode, one-shot prompt mode, terminal rendering |

### 4.5 Rust Workspace Config

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

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

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_panics_doc = "allow"
missing_errors_doc = "allow"
```

---

## 5. Scraper Service (Python)

A lightweight **FastAPI** microservice that wraps Scrapling for Ctrip-specific scraping:

```mermaid
flowchart LR
    A["Rust Agent"] -->|"HTTP POST"| B["FastAPI :8300"]
    B --> C["StealthyFetcher"]
    C --> D{"Anti-Bot Strategy"}
    D --> E["TLS Impersonation"]
    D --> F["Canvas Noise"]
    D --> G["WebRTC Blocking"]
    D --> H["Cloudflare Solver"]
    E --> I["Patchright Browser"]
    F --> I
    G --> I
    H --> I
    I -->|"Proxy Rotation"| J["Ctrip Page"]
    J --> K["CtripParser"]
    K --> L["Hotel Data JSON"]
    L --> M["Write to SQLite"]
```

### Ctrip Scraping Strategy

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

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scrape/hotels` | Scrape hotel list for city + dates |
| `POST` | `/scrape/hotel/{id}` | Scrape single hotel detail page |
| `GET` | `/health` | Health check |
| `GET` | `/sessions` | List active browser sessions |

---

## 6. Web Frontend (Next.js)

### Pages

| Route | Description |
|-------|-------------|
| `/` | Search form — select city, dates, filters |
| `/hotels` | Hotel list — cards with name, price, rating, photo |
| `/hotels/[id]` | Hotel detail — room types, price history chart, amenities |

### Key Components

- **SearchForm** — City autocomplete, date picker, guest count, star filter
- **HotelCard** — Compact hotel preview with key info and lowest price
- **PriceChart** — Line chart showing price trends over time (recharts)
- **FilterPanel** — Price range slider, star rating, distance, amenities
- **DataTable** — Sortable, paginated table view of all scraped data

### Data Flow

```mermaid
flowchart LR
    A["Frontend Pages"] --> B["Next.js API Routes"]
    B --> C["Rust CLI"]
    C --> D["SQLite\nRead Scraped Data"]
    C --> E["Scraper Service\nTrigger New Scrapes"]
```

---

## 7. Data Model

### Hotel

```rust
pub struct Hotel {
    pub id: String,              // Ctrip hotel ID
    pub name: String,
    pub name_en: Option<String>,
    pub star: u8,                // 1-5
    pub rating: f64,             // User rating (0-5.0)
    pub rating_count: u32,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub image_url: Option<String>,
    pub amenities: Vec<String>,
    pub city: String,
    pub district: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Room

```rust
pub struct Room {
    pub id: String,
    pub hotel_id: String,
    pub name: String,            // "大床房", "双床房", etc.
    pub bed_type: Option<String>,
    pub max_guests: u8,
    pub area: Option<f64>,       // sqm
    pub has_window: bool,
    pub has_breakfast: bool,
    pub cancellation_policy: Option<String>,
}
```

### PriceSnapshot

```rust
pub struct PriceSnapshot {
    pub id: String,
    pub room_id: String,
    pub hotel_id: String,
    pub price: f64,              // CNY per night
    pub original_price: Option<f64>,
    pub checkin: NaiveDate,
    pub checkout: NaiveDate,
    pub scraped_at: DateTime<Utc>,
    pub source: String,          // "ctrip"
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

## 8. Agent Tool Definitions

Following claw-code's `ToolSpec` pattern — each tool has name, description, JSON Schema, and a typed execute handler:

### `scrape_hotels`

```json
{
    "name": "scrape_hotels",
    "description": "Scrape hotel listings from Ctrip for a given city and date range. Calls the Python scraper service to handle anti-bot bypass and browser automation.",
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
    "description": "Search previously scraped hotel data from local SQLite database.",
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
    "description": "Analyze price trends and compare hotels across multiple snapshots.",
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
    "description": "Export scraped data as CSV or JSON file.",
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

## 9. Build & Dev Workflow

### Prerequisites

- Rust 1.80+ (with `cargo`)
- Python 3.10+ (with `uv` or `pip`)
- Node.js 20+ (with `pnpm`)
- Chromium (installed by Playwright/Patchright)

### Turborepo Pipeline

```json
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
cargo build --workspace           # Rust workspace
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

## 10. Configuration

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

## 11. Tech Stack Summary

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Agent Runtime | Rust (tokio, reqwest, clap, rustyline) | Task orchestration, tool execution, REPL |
| LLM Provider | Anthropic API (SSE streaming) | Agent intelligence |
| Scraper | Python (Scrapling, FastAPI, Patchright) | Stealth web scraping with anti-bot bypass |
| Storage | SQLite (rusqlite) | Hotel & price data persistence |
| Frontend | Next.js 15, Tailwind CSS, Recharts | Dashboard UI |
| Monorepo | Turborepo + pnpm + Cargo workspace | Build orchestration |

---

## 12. Roadmap

### Phase 1 — MVP
- [ ] Project scaffolding (monorepo, configs, Cargo workspace)
- [ ] Python scraper service with Ctrip hotel list parsing
- [ ] Rust storage layer (SQLite)
- [ ] Basic CLI with `scrape` and `search` commands
- [ ] Minimal frontend with hotel list view

### Phase 2 — Agent Intelligence
- [ ] Full agent loop (`ConversationRuntime<C,T>`) with LLM integration
- [ ] Tool definitions (scrape, search, analyze, export)
- [ ] REPL mode with session persistence
- [ ] Price comparison and trend analysis

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
