use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        // v0.1 tables (hotels)
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS hotels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                name_en TEXT,
                star INTEGER,
                rating REAL,
                rating_count INTEGER DEFAULT 0,
                address TEXT,
                latitude REAL,
                longitude REAL,
                image_url TEXT,
                amenities TEXT DEFAULT '[]',
                city TEXT NOT NULL,
                district TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS rooms (
                id TEXT PRIMARY KEY,
                hotel_id TEXT NOT NULL REFERENCES hotels(id),
                name TEXT NOT NULL,
                bed_type TEXT,
                max_guests INTEGER DEFAULT 2,
                area REAL,
                has_window INTEGER DEFAULT 0,
                has_breakfast INTEGER DEFAULT 0,
                cancellation_policy TEXT
            );

            CREATE TABLE IF NOT EXISTS price_snapshots (
                id TEXT PRIMARY KEY,
                room_id TEXT NOT NULL REFERENCES rooms(id),
                hotel_id TEXT NOT NULL REFERENCES hotels(id),
                price REAL NOT NULL,
                original_price REAL,
                checkin TEXT NOT NULL,
                checkout TEXT NOT NULL,
                scraped_at TEXT NOT NULL DEFAULT (datetime('now')),
                source TEXT DEFAULT 'ctrip'
            );

            CREATE INDEX IF NOT EXISTS idx_prices_hotel ON price_snapshots(hotel_id);
            CREATE INDEX IF NOT EXISTS idx_prices_date ON price_snapshots(checkin, checkout);
            CREATE INDEX IF NOT EXISTS idx_prices_scraped ON price_snapshots(scraped_at);
            CREATE INDEX IF NOT EXISTS idx_hotels_city ON hotels(city);
            ",
        )?;

        // v0.2 tables (transport and geography)
        self.conn.execute_batch(include_str!("../migrations/001_add_transport_and_geo_tables.sql"))?;
        self.conn.execute_batch(include_str!("../migrations/002_add_geo_lookup_tables.sql"))?;

        // v0.3 tables (price monitoring)
        self.conn.execute_batch(include_str!("../migrations/003_add_price_subscriptions.sql"))?;

        // v0.3 performance indexes
        self.conn.execute_batch(include_str!("../migrations/004_add_performance_indexes.sql"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_init_with_migrations() {
        let db = Database::open_in_memory().expect("Failed to create in-memory database");

        // 验证 v0.1 表存在
        let hotel_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hotels'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(hotel_count, 1, "hotels table should exist");

        // 验证 v0.2 火车票表存在
        let train_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='trains'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(train_count, 1, "trains table should exist");

        // 验证 v0.2 机票表存在
        let flight_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='flights'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(flight_count, 1, "flights table should exist");

        // 验证 v0.2 城市表存在
        let city_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cities'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(city_count, 1, "cities table should exist");

        // 验证 v0.2 区域表存在
        let district_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='districts'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(district_count, 1, "districts table should exist");

        // 验证 v0.2 景点表存在
        let attraction_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='attractions'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(attraction_count, 1, "attractions table should exist");

        // 验证 v0.2 知识维基表存在
        let wiki_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wiki_entries'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(wiki_count, 1, "wiki_entries table should exist");

        // 验证 v0.3 城市映射表存在
        let city_mappings_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='city_mappings'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(city_mappings_count, 1, "city_mappings table should exist");

        // 验证 v0.3 车站代码表存在
        let station_codes_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='station_codes'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(station_codes_count, 1, "station_codes table should exist");

        // 验证 v0.3 机场代码表存在
        let airport_codes_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='airport_codes'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(airport_codes_count, 1, "airport_codes table should exist");

        // Verify price_subscriptions table exists (migration 003)
        let subs_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='price_subscriptions'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(subs_count, 1, "price_subscriptions table should exist");

        // Verify performance indexes exist (migration 004)
        let idx_count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert!(idx_count >= 15, "should have at least 15 indexes, got {idx_count}");
    }

    #[test]
    fn test_train_crud() {
        let db = Database::open_in_memory().unwrap();
        let train = crate::models::Train {
            id: "G1234".to_string(),
            train_type: "G".to_string(),
            from_station: "北京南".to_string(),
            to_station: "上海虹桥".to_string(),
            from_city: "北京".to_string(),
            to_city: "上海".to_string(),
            depart_time: "08:00".to_string(),
            arrive_time: "12:30".to_string(),
            duration_minutes: 270,
            distance_km: Some(1318),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        db.upsert_train(&train).unwrap();

        let price = crate::models::TrainPrice {
            id: "tp-001".to_string(),
            train_id: "G1234".to_string(),
            seat_type: "二等座".to_string(),
            price: 553.0,
            available_seats: Some(100),
            travel_date: "2026-05-01".to_string(),
            scraped_at: chrono::Utc::now().to_rfc3339(),
            source: "12306".to_string(),
        };
        db.insert_train_price(&price).unwrap();

        let results = db.search_trains("北京", "上海", "2026-05-01", 120).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].train.id, "G1234");
        assert_eq!(results[0].lowest_price, Some(553.0));
    }

    #[test]
    fn test_flight_crud() {
        let db = Database::open_in_memory().unwrap();
        let flight = crate::models::Flight {
            id: "CA1234".to_string(),
            airline: "中国国航".to_string(),
            from_airport: "PEK".to_string(),
            to_airport: "SHA".to_string(),
            from_city: "北京".to_string(),
            to_city: "上海".to_string(),
            depart_time: "10:00".to_string(),
            arrive_time: "12:15".to_string(),
            duration_minutes: 135,
            aircraft_type: Some("A320".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        db.upsert_flight(&flight).unwrap();

        let price = crate::models::FlightPrice {
            id: "fp-001".to_string(),
            flight_id: "CA1234".to_string(),
            cabin_class: "经济舱".to_string(),
            price: 800.0,
            discount: Some(0.8),
            available_seats: Some(50),
            travel_date: "2026-05-01".to_string(),
            scraped_at: chrono::Utc::now().to_rfc3339(),
            source: "mock".to_string(),
        };
        db.insert_flight_price(&price).unwrap();

        let results = db.search_flights("北京", "上海", "2026-05-01", 120).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].flight.id, "CA1234");
        assert_eq!(results[0].lowest_price, Some(800.0));
    }

    #[test]
    fn test_wiki_crud() {
        let db = Database::open_in_memory().unwrap();

        // Remember
        db.upsert_wiki_entry("w1", "user_history", "budget", "500-1000", None, "2026-01-01T00:00:00Z").unwrap();
        db.upsert_wiki_entry("w2", "user_history", "star", "4", None, "2026-01-01T00:00:00Z").unwrap();
        db.upsert_wiki_entry("w3", "city_guide", "beijing", "故宫长城", None, "2026-01-01T00:00:00Z").unwrap();

        // Recall
        let entry = db.get_wiki_entry("user_history", "budget").unwrap().unwrap();
        assert_eq!(entry.value, "500-1000");

        // List by topic
        let entries = db.list_wiki_entries(Some("user_history")).unwrap();
        assert_eq!(entries.len(), 2);

        // List all
        let all = db.list_wiki_entries(None).unwrap();
        assert_eq!(all.len(), 3);

        // Update (upsert)
        db.upsert_wiki_entry("w1-new", "user_history", "budget", "800-1500", None, "2026-01-02T00:00:00Z").unwrap();
        let updated = db.get_wiki_entry("user_history", "budget").unwrap().unwrap();
        assert_eq!(updated.value, "800-1500");

        // Forget
        db.delete_wiki_entry("user_history", "budget").unwrap();
        let deleted = db.get_wiki_entry("user_history", "budget").unwrap();
        assert!(deleted.is_none());

        // Remaining entries
        let remaining = db.list_wiki_entries(None).unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn test_price_subscription_crud() {
        let db = Database::open_in_memory().unwrap();

        // Subscribe
        db.upsert_price_subscription("sub1", "user1", "北京", "上海", "train", 500.0, "2026-01-01", "2026-01-08").unwrap();
        db.upsert_price_subscription("sub2", "user1", "北京", "广州", "flight", 1000.0, "2026-01-01", "2026-01-08").unwrap();

        // List
        let subs = db.list_active_subscriptions(Some("user1")).unwrap();
        assert_eq!(subs.len(), 2);

        // Unsubscribe
        db.deactivate_subscription("sub1").unwrap();
        let subs = db.list_active_subscriptions(Some("user1")).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0]["id"], "sub2");
    }

    #[test]
    fn test_hotel_search_filters() {
        let db = Database::open_in_memory().unwrap();

        // Insert test hotels
        let hotels = vec![
            ("h1", "Budget Inn", 2, 3.5, "北京"),
            ("h2", "Mid Hotel", 3, 4.0, "北京"),
            ("h3", "Luxury Resort", 5, 4.8, "上海"),
        ];
        for (id, name, star, rating, city) in &hotels {
            let h = crate::models::Hotel {
                id: id.to_string(),
                name: name.to_string(),
                name_en: None,
                star: Some(*star),
                rating: Some(*rating),
                rating_count: 100,
                address: None,
                latitude: None,
                longitude: None,
                image_url: None,
                amenities: vec![],
                city: city.to_string(),
                district: None,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
            };
            db.upsert_hotel(&h).unwrap();
        }

        // Filter by city
        let filters = crate::models::SearchFilters {
            city: Some("北京".to_string()),
            ..Default::default()
        };
        let results = db.search_hotels(&filters).unwrap();
        assert_eq!(results.len(), 2);

        // Filter by min star
        let filters = crate::models::SearchFilters {
            min_star: Some(4),
            ..Default::default()
        };
        let results = db.search_hotels(&filters).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].hotel.name, "Luxury Resort");

        // Filter by min rating
        let filters = crate::models::SearchFilters {
            min_rating: Some(4.0),
            ..Default::default()
        };
        let results = db.search_hotels(&filters).unwrap();
        assert_eq!(results.len(), 2);
    }
}
