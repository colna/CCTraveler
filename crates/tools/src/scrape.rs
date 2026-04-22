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
