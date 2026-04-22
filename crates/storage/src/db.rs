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
        Ok(())
    }
}
