//! 老的命令式子命令：scrape / search / export。
//! 行为完全保留，从原 main.rs 迁移过来。

use anyhow::Result;
use runtime::RuntimeConfig;
use std::path::{Path, PathBuf};
use storage::models::{SearchFilters, SortBy};
use storage::Database;
use tools::scrape::{ScrapeRequest, ScrapedHotel};

pub async fn scrape(
    config: &RuntimeConfig,
    db_path: &Path,
    city: String,
    checkin: String,
    checkout: String,
    max_pages: u32,
) -> Result<()> {
    let db = Database::open(db_path)?;
    println!("Scraping hotels: city={city}, {checkin} to {checkout}, max_pages={max_pages}");

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
    Ok(())
}

pub fn search(
    db_path: &Path,
    city: Option<String>,
    max_price: Option<f64>,
    min_star: Option<u8>,
    min_rating: Option<f64>,
    sort_by: Option<String>,
    limit: usize,
) -> Result<()> {
    let db = Database::open(db_path)?;
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
        println!(
            "No hotels found. Try scraping first with: \
             cctraveler scrape --city <city> --checkin <date> --checkout <date>"
        );
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
    Ok(())
}

pub fn export(
    db_path: &Path,
    format: String,
    city: Option<String>,
    output: Option<PathBuf>,
) -> Result<()> {
    let db = Database::open(db_path)?;
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
    Ok(())
}

fn print_hotel_table(hotels: &[ScrapedHotel]) {
    println!(
        "\n{:<30} {:>5} {:>6} {:>10}  Room",
        "Name", "Star", "Rating", "Price"
    );
    println!("{}", "-".repeat(80));
    for h in hotels {
        let min_price = h.rooms.iter().filter_map(|r| r.price).reduce(f64::min);
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
