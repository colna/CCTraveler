use anyhow::Result;
use storage::models::SearchFilters;
use storage::Database;

pub fn export_json(db: &Database, filters: &SearchFilters) -> Result<String> {
    db.export_json(filters)
}

pub fn export_csv(db: &Database, filters: &SearchFilters) -> Result<String> {
    db.export_csv(filters)
}
