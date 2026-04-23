use serde::{Deserialize, Serialize};

// ============================================================
// Hotel models (v0.1)
// ============================================================

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

// ============================================================
// Train models (v0.2)
// ============================================================

/// 火车票信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Train {
    pub id: String,              // 车次号 (如 G1234)
    pub train_type: String,      // 车型 (G/D/C/K/T/Z)
    pub from_station: String,    // 出发站
    pub to_station: String,      // 到达站
    pub from_city: String,       // 出发城市
    pub to_city: String,         // 到达城市
    pub depart_time: String,     // 出发时间 (HH:MM)
    pub arrive_time: String,     // 到达时间 (HH:MM)
    pub duration_minutes: i32,   // 运行时长（分钟）
    pub distance_km: Option<i32>,// 里程（公里）
    pub created_at: String,
    pub updated_at: String,
}

/// 火车票价格快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainPrice {
    pub id: String,
    pub train_id: String,
    pub seat_type: String,       // 座位类型 (商务座/一等座/二等座/硬卧/软卧)
    pub price: f64,              // 价格（元）
    pub available_seats: Option<i32>, // 余票数量 (-1 表示未知)
    pub travel_date: String,     // 乘车日期 (YYYY-MM-DD)
    pub scraped_at: String,
    pub source: String,          // 数据来源 (12306)
}

/// 火车票搜索结果（带价格信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainSearchResult {
    pub train: Train,
    pub lowest_price: Option<f64>,
    pub seat_type: Option<String>,
    pub available_seats: Option<i32>,
}

// ============================================================
// Flight models (v0.2)
// ============================================================

/// 机票信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flight {
    pub id: String,              // 航班号 (如 CA1234)
    pub airline: String,         // 航空公司
    pub from_airport: String,    // 出发机场代码 (如 PEK)
    pub to_airport: String,      // 到达机场代码 (如 SHA)
    pub from_city: String,       // 出发城市
    pub to_city: String,         // 到达城市
    pub depart_time: String,     // 出发时间 (HH:MM)
    pub arrive_time: String,     // 到达时间 (HH:MM)
    pub duration_minutes: i32,   // 飞行时长（分钟）
    pub aircraft_type: Option<String>, // 机型 (如 A320)
    pub created_at: String,
    pub updated_at: String,
}

/// 机票价格快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightPrice {
    pub id: String,
    pub flight_id: String,
    pub cabin_class: String,     // 舱位等级 (头等舱/商务舱/经济舱)
    pub price: f64,              // 价格（元）
    pub discount: Option<f64>,   // 折扣 (如 0.8 表示 8 折)
    pub available_seats: Option<i32>, // 余票数量
    pub travel_date: String,     // 出行日期 (YYYY-MM-DD)
    pub scraped_at: String,
    pub source: String,          // 数据来源 (ctrip/qunar/fliggy)
}

/// 机票搜索结果（带价格信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightSearchResult {
    pub flight: Flight,
    pub lowest_price: Option<f64>,
    pub cabin_class: Option<String>,
    pub discount: Option<f64>,
    pub available_seats: Option<i32>,
}

// ============================================================
// Geography models (v0.2)
// ============================================================

/// 城市信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct City {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub province: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub population: Option<i32>,
    pub area_km2: Option<f64>,
    pub tier: Option<i32>,       // 城市等级 (1/2/3/4/5)
    pub description: Option<String>,
    pub created_at: String,
}

/// 城市区域
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct District {
    pub id: String,
    pub city_id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub description: Option<String>,
    pub tags: Option<String>,    // JSON 数组: ["商业区","交通枢纽"]
}

/// 景点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attraction {
    pub id: String,
    pub city_id: String,
    pub district_id: Option<String>,
    pub name: String,
    pub name_en: Option<String>,
    pub category: String,        // 类别 (历史/自然/娱乐/购物)
    pub rating: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub address: Option<String>,
    pub description: Option<String>,
    pub opening_hours: Option<String>,
    pub ticket_price: Option<f64>,
    pub visit_duration_hours: Option<f64>,
}

// ============================================================
// Wiki models (v0.2)
// ============================================================

/// 知识维基条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiEntry {
    pub id: String,
    pub topic: String,           // 主题 (user_history/city_guide/route_tips)
    pub key: String,             // 键 (如 "user_budget_range")
    pub value: String,           // 值 (JSON 格式)
    pub metadata: Option<String>,// 元数据 (JSON)
    pub created_at: String,
    pub updated_at: String,
}
