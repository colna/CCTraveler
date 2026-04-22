use anyhow::Result;
use storage::models::{HotelWithPrice, SearchFilters};
use storage::Database;

pub fn search_hotels(db: &Database, filters: &SearchFilters) -> Result<Vec<HotelWithPrice>> {
    db.search_hotels(filters)
}
