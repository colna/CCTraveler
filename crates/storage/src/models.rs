use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotel {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub star: Option<u8>,
    pub rating: Option<f64>,
    pub rating_count: u32,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub image_url: Option<String>,
    pub amenities: Vec<String>,
    pub city: String,
    pub district: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub hotel_id: String,
    pub name: String,
    pub bed_type: Option<String>,
    pub max_guests: u8,
    pub area: Option<f64>,
    pub has_window: bool,
    pub has_breakfast: bool,
    pub cancellation_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub id: String,
    pub room_id: String,
    pub hotel_id: String,
    pub price: f64,
    pub original_price: Option<f64>,
    pub checkin: String,
    pub checkout: String,
    pub scraped_at: String,
    pub source: String,
}

/// Joined hotel + lowest price for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotelWithPrice {
    pub hotel: Hotel,
    pub lowest_price: Option<f64>,
    pub original_price: Option<f64>,
    pub room_name: Option<String>,
}

/// Search filters for hotel queries
#[derive(Debug, Default)]
pub struct SearchFilters {
    pub city: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub min_star: Option<u8>,
    pub min_rating: Option<f64>,
    pub sort_by: Option<SortBy>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SortBy {
    Price,
    Rating,
    Star,
}
