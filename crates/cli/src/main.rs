use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use storage::models::{Hotel, PriceSnapshot, Room, SearchFilters, SortBy};
use storage::Database;
use tools::scrape::{ScrapeRequest, ScrapedHotel};

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

    let db = Database::open(&db_path)?;

    match cli.command {
        Commands::Scrape {
            city,
            checkin,
            checkout,
            max_pages,
        } => {
            println!("Scraping hotels: city={city}, {checkin} to {checkout}, max_pages={max_pages}");

            let req = ScrapeRequest {
                city: city.clone(),
                checkin: checkin.clone(),
                checkout: checkout.clone(),
                max_pages,
            };

            let resp = tools::scrape::scrape_hotels(&config.scraper.base_url, &req).await?;
            println!("Got {} hotels from scraper", resp.hotels.len());

            let now = chrono::Utc::now().to_rfc3339();
            for hotel in &resp.hotels {
                store_scraped_hotel(&db, hotel, &city, &checkin, &checkout, &now)?;
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
                "{:<12} {:<30} {:>5} {:>6} {:>8} {}",
                "ID", "Name", "Star", "Rating", "Price", "Room"
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

fn store_scraped_hotel(
    db: &Database,
    hotel: &ScrapedHotel,
    city: &str,
    checkin: &str,
    checkout: &str,
    now: &str,
) -> Result<()> {
    let h = Hotel {
        id: hotel.id.clone(),
        name: hotel.name.clone(),
        name_en: hotel.name_en.clone(),
        star: hotel.star,
        rating: hotel.rating,
        rating_count: hotel.rating_count.unwrap_or(0),
        address: hotel.address.clone(),
        latitude: hotel.latitude,
        longitude: hotel.longitude,
        image_url: hotel.image_url.clone(),
        amenities: vec![],
        city: city.to_string(),
        district: hotel.district.clone(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    };
    db.upsert_hotel(&h)?;

    for (i, room) in hotel.rooms.iter().enumerate() {
        let room_id = format!("{}-room-{i}", hotel.id);
        let r = Room {
            id: room_id.clone(),
            hotel_id: hotel.id.clone(),
            name: room.name.clone(),
            bed_type: room.bed_type.clone(),
            max_guests: 2,
            area: None,
            has_window: room.has_window.unwrap_or(false),
            has_breakfast: room.has_breakfast.unwrap_or(false),
            cancellation_policy: None,
        };
        db.insert_room(&r)?;

        let price = PriceSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            room_id,
            hotel_id: hotel.id.clone(),
            price: room.price,
            original_price: room.original_price,
            checkin: checkin.to_string(),
            checkout: checkout.to_string(),
            scraped_at: now.to_string(),
            source: "ctrip".to_string(),
        };
        db.insert_price(&price)?;
    }

    Ok(())
}

fn print_hotel_table(hotels: &[ScrapedHotel]) {
    println!(
        "\n{:<30} {:>5} {:>6} {:>10}",
        "Name", "Star", "Rating", "Min Price"
    );
    println!("{}", "-".repeat(55));
    for h in hotels {
        let min_price = h
            .rooms
            .iter()
            .map(|r| r.price)
            .reduce(f64::min)
            .unwrap_or(0.0);
        println!(
            "{:<30} {:>5} {:>6} {:>10}",
            truncate(&h.name, 30),
            h.star.map_or("-".to_string(), |s| format!("{s}")),
            h.rating.map_or("-".to_string(), |r| format!("{r:.1}")),
            format!("¥{min_price:.0}"),
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
