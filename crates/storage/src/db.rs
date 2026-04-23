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
    }
}
