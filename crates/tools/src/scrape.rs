use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ScrapeRequest {
    pub city: String,
    pub checkin: String,
    pub checkout: String,
    pub max_pages: u32,
    #[serde(default = "default_source")]
    pub source: String,
}

#[allow(dead_code)]
fn default_source() -> String {
    "trip".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ScrapeResponse {
    pub hotels: Vec<ScrapedHotel>,
    pub total: usize,
    pub scraped_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedHotel {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub star: Option<u8>,
    pub rating: Option<f64>,
    pub rating_count: Option<u32>,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub image_url: Option<String>,
    pub city: String,
    pub district: Option<String>,
    pub rooms: Vec<ScrapedRoom>,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedRoom {
    pub name: String,
    pub price: Option<f64>,
    pub original_price: Option<f64>,
    pub bed_type: Option<String>,
    pub has_breakfast: Option<bool>,
    pub has_free_cancel: Option<bool>,
}

/// Call the Python scraper service via HTTP
pub async fn scrape_hotels(base_url: &str, req: &ScrapeRequest) -> Result<ScrapeResponse> {
    let client = reqwest::Client::new();
    let url = format!("{base_url}/scrape/hotels");
    let resp = client
        .post(&url)
        .json(req)
        .timeout(std::time::Duration::from_mins(2))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Scraper returned {status}: {body}");
    }

    let data: ScrapeResponse = resp.json().await?;
    Ok(data)
}

// ============================================================
// Train scraping
// ============================================================

#[derive(Debug, Serialize)]
pub struct TrainScrapeRequest {
    pub from_city: String,
    pub to_city: String,
    pub travel_date: String,
}

#[derive(Debug, Deserialize)]
pub struct TrainScrapeResponse {
    pub trains: Vec<ScrapedTrain>,
    pub total: usize,
    pub scraped_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedTrain {
    pub train_id: String,
    pub train_type: String,
    pub from_station: String,
    pub to_station: String,
    pub from_city: String,
    pub to_city: String,
    pub depart_time: String,
    pub arrive_time: String,
    pub duration_minutes: i32,
    pub distance_km: Option<i32>,
    pub seats: Vec<ScrapedTrainSeat>,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedTrainSeat {
    pub seat_type: String,
    pub price: f64,
    pub available_seats: Option<i32>,
}

pub async fn scrape_trains(
    base_url: &str,
    from_city: &str,
    to_city: &str,
    travel_date: &str,
) -> Result<Vec<ScrapedTrain>> {
    let client = reqwest::Client::new();
    let url = format!("{base_url}/scrape/trains");
    let req = TrainScrapeRequest {
        from_city: from_city.to_string(),
        to_city: to_city.to_string(),
        travel_date: travel_date.to_string(),
    };

    let resp = client
        .post(&url)
        .json(&req)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Scraper returned {status}: {body}");
    }

    let data: TrainScrapeResponse = resp.json().await?;
    Ok(data.trains)
}

// ============================================================
// Flight scraping
// ============================================================

#[derive(Debug, Serialize)]
pub struct FlightScrapeRequest {
    pub from_city: String,
    pub to_city: String,
    pub travel_date: String,
}

#[derive(Debug, Deserialize)]
pub struct FlightScrapeResponse {
    pub flights: Vec<ScrapedFlight>,
    pub total: usize,
    pub scraped_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedFlight {
    pub flight_id: String,
    pub airline: String,
    pub from_airport: String,
    pub to_airport: String,
    pub from_city: String,
    pub to_city: String,
    pub depart_time: String,
    pub arrive_time: String,
    pub duration_minutes: i32,
    pub aircraft_type: Option<String>,
    pub prices: Vec<ScrapedFlightPrice>,
}

#[derive(Debug, Deserialize)]
pub struct ScrapedFlightPrice {
    pub cabin_class: String,
    pub price: f64,
    pub discount: Option<f64>,
    pub available_seats: Option<i32>,
}

pub async fn scrape_flights(
    base_url: &str,
    from_city: &str,
    to_city: &str,
    travel_date: &str,
) -> Result<Vec<ScrapedFlight>> {
    let client = reqwest::Client::new();
    let url = format!("{base_url}/scrape/flights");
    let req = FlightScrapeRequest {
        from_city: from_city.to_string(),
        to_city: to_city.to_string(),
        travel_date: travel_date.to_string(),
    };

    let resp = client
        .post(&url)
        .json(&req)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Scraper returned {status}: {body}");
    }

    let data: FlightScrapeResponse = resp.json().await?;
    Ok(data.flights)
}
