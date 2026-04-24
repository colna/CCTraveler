use anyhow::Result;
use api::AnthropicRuntimeClient;
use clap::{Parser, Subcommand};
use runtime::{ConversationRuntime, SystemPromptBuilder};
use rustyline::DefaultEditor;
use std::path::PathBuf;
use storage::models::{SearchFilters, SortBy};
use storage::Database;
use tools::scrape::{ScrapeRequest, ScrapedHotel};
use tools::TravelerToolExecutor;

#[derive(Parser)]
#[command(name = "cctraveler", version, about = "AI Travel Planner — Hotel Price Intelligence")]
struct Cli {
    /// Path to config file
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start AI chat mode (natural language agent)
    Chat,
    /// Scrape hotel listings from Ctrip
    Scrape {
        /// City name or Ctrip city ID
        #[arg(long)]
        city: String,
        /// Check-in date (YYYY-MM-DD)
        #[arg(long)]
        checkin: String,
        /// Check-out date (YYYY-MM-DD)
        #[arg(long)]
        checkout: String,
        /// Max pages to scrape
        #[arg(long, default_value = "5")]
        max_pages: u32,
    },
    /// Search previously scraped hotels
    Search {
        /// Filter by city
        #[arg(long)]
        city: Option<String>,
        /// Maximum price per night
        #[arg(long)]
        max_price: Option<f64>,
        /// Minimum star rating
        #[arg(long)]
        min_star: Option<u8>,
        /// Minimum user rating
        #[arg(long)]
        min_rating: Option<f64>,
        /// Sort by: price, rating, star
        #[arg(long)]
        sort_by: Option<String>,
        /// Limit results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Export scraped data
    Export {
        /// Output format: csv or json
        #[arg(long)]
        format: String,
        /// Filter by city
        #[arg(long)]
        city: Option<String>,
        /// Output file path
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = runtime::RuntimeConfig::load(&cli.config)?;
    let db_path = PathBuf::from(&config.storage.db_path);

    // Ensure data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match cli.command {
        Commands::Chat => {
            run_chat(&config, &db_path)?;
        }
        Commands::Scrape {
            city,
            checkin,
            checkout,
            max_pages,
        } => {
            let db = Database::open(&db_path)?;
            println!(
                "Scraping hotels: city={city}, {checkin} to {checkout}, max_pages={max_pages}"
            );

            let req = ScrapeRequest {
                city: city.clone(),
                checkin: checkin.clone(),
                checkout: checkout.clone(),
                max_pages,
                source: "trip".to_string(),
            };

            let resp = tools::scrape::scrape_hotels(&config.scraper.base_url, &req).await?;
            println!("Got {} hotels from scraper", resp.hotels.len());

            let now = chrono::Utc::now().to_rfc3339();
            for hotel in &resp.hotels {
                tools::store_scraped_hotel(&db, hotel, &city, &checkin, &checkout, &now)?;
            }

            println!("Stored {} hotels in database", resp.hotels.len());
            print_hotel_table(&resp.hotels);
        }
        Commands::Search {
            city,
            max_price,
            min_star,
            min_rating,
            sort_by,
            limit,
        } => {
            let db = Database::open(&db_path)?;
            let filters = SearchFilters {
                city,
                max_price,
                min_star,
                min_rating,
                sort_by: sort_by.as_deref().map(parse_sort_by),
                limit: Some(limit),
                ..Default::default()
            };
            let results = db.search_hotels(&filters)?;

            if results.is_empty() {
                println!("No hotels found. Try scraping first with: cctraveler scrape --city <city> --checkin <date> --checkout <date>");
                return Ok(());
            }

            println!("Found {} hotels:\n", results.len());
            println!(
                "{:<12} {:<30} {:>5} {:>6} {:>8} Room",
                "ID", "Name", "Star", "Rating", "Price"
            );
            println!("{}", "-".repeat(85));
            for h in &results {
                println!(
                    "{:<12} {:<30} {:>5} {:>6} {:>8} {}",
                    &h.hotel.id[..h.hotel.id.len().min(12)],
                    truncate(&h.hotel.name, 30),
                    h.hotel.star.map_or("-".to_string(), |s| format!("{s}")),
                    h.hotel
                        .rating
                        .map_or("-".to_string(), |r| format!("{r:.1}")),
                    h.lowest_price
                        .map_or("-".to_string(), |p| format!("¥{p:.0}")),
                    h.room_name.as_deref().unwrap_or("-"),
                );
            }
        }
        Commands::Export {
            format,
            city,
            output,
        } => {
            let db = Database::open(&db_path)?;
            let filters = SearchFilters {
                city,
                ..Default::default()
            };
            let content = match format.as_str() {
                "csv" => db.export_csv(&filters)?,
                "json" => db.export_json(&filters)?,
                other => anyhow::bail!("Unsupported format: {other}. Use csv or json."),
            };

            if let Some(path) = output {
                std::fs::write(&path, &content)?;
                println!("Exported to {}", path.display());
            } else {
                print!("{content}");
            }
        }
    }

    Ok(())
}

/// Run the AI chat REPL — `ConversationRuntime`<`AnthropicRuntimeClient`, `TravelerToolExecutor`>
fn run_chat(config: &runtime::RuntimeConfig, db_path: &std::path::Path) -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║   CCTraveler AI 旅行助手               ║");
    println!("║   输入自然语言查询酒店信息               ║");
    println!("║   输入 quit 或 exit 退出                ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    // Initialize Prometheus metrics
    tools::metrics::init_metrics();

    // Initialize API client: config.toml > env var
    let api_key = config.agent.resolve_api_key().ok_or_else(|| {
        anyhow::anyhow!(
            "API key not found.\n\
             Set api_key in config.toml [agent] section, or set ANTHROPIC_API_KEY env var."
        )
    })?;
    let base_url = config.agent.resolve_base_url();
    let api_client = AnthropicRuntimeClient::with_base_url(api_key, base_url);

    // Initialize tool executor with its own database connection and optional Redis cache
    let db = Database::open(db_path)?;
    let redis = tools::cache::RedisCache::new(
        config.redis.enabled,
        &config.redis.url,
        config.redis.ttl_seconds,
    );
    let tool_executor = TravelerToolExecutor::new(db, config.scraper.base_url.clone())
        .with_redis(redis);

    // Start background price scheduler (every hour)
    let _scheduler_handle = tools::scheduler::PriceScheduler::new(
        db_path.to_path_buf(),
        config.scraper.base_url.clone(),
        3600,
    )
    .spawn();

    // Build system prompt
    let system_prompt = SystemPromptBuilder::build_default();

    // Create ConversationRuntime
    let mut rt = ConversationRuntime::new(
        api_client,
        tool_executor,
        config.agent.model.clone(),
        system_prompt,
        config.agent.max_turns as usize,
    );

    // Set workspace root for session persistence
    let cwd = std::env::current_dir()?;
    rt.session.workspace_root = Some(cwd);

    // REPL loop with rustyline
    let mut editor = DefaultEditor::new()?;
    let history_path = dirs_hint();
    if let Some(ref path) = history_path {
        let _ = editor.load_history(path);
    }

    loop {
        let input = match editor.readline("you> ") {
            Ok(line) => line,
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("(Ctrl+C) 输入 quit 退出");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let _ = editor.add_history_entry(input);

        if matches!(input, "quit" | "exit" | "q") {
            println!("再见！");
            break;
        }

        // Run a turn
        match rt.run_turn(input) {
            Ok(summary) => {
                println!("\nassistant> {}", summary.assistant_text);
                if summary.tool_calls_made > 0 {
                    println!(
                        "  [工具调用: {} 次 | tokens: {} in / {} out]",
                        summary.tool_calls_made, summary.input_tokens, summary.output_tokens
                    );
                }
                println!();
            }
            Err(e) => {
                eprintln!("Error: {e}");
                println!();
            }
        }
    }

    // Save session
    if let Err(e) = rt.save_session() {
        eprintln!("Warning: failed to save session: {e}");
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = editor.save_history(path);
    }

    Ok(())
}

fn dirs_hint() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".cctraveler");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("history.txt"))
}

fn print_hotel_table(hotels: &[ScrapedHotel]) {
    println!(
        "\n{:<30} {:>5} {:>6} {:>10}  Room",
        "Name", "Star", "Rating", "Price"
    );
    println!("{}", "-".repeat(80));
    for h in hotels {
        let min_price = h
            .rooms
            .iter()
            .filter_map(|r| r.price)
            .reduce(f64::min);
        let room_name = h.rooms.first().map_or("-", |r| r.name.as_str());
        println!(
            "{:<30} {:>5} {:>6} {:>10}  {}",
            truncate(&h.name, 30),
            h.star.map_or("-".to_string(), |s| format!("{s}")),
            h.rating.map_or("-".to_string(), |r| format!("{r:.1}")),
            min_price.map_or("-".to_string(), |p| format!("¥{p:.0}")),
            truncate(room_name, 25),
        );
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 2).collect();
        format!("{truncated}..")
    }
}

fn parse_sort_by(s: &str) -> SortBy {
    match s {
        "rating" => SortBy::Rating,
        "star" => SortBy::Star,
        _ => SortBy::Price,
    }
}
