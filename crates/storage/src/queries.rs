use crate::db::Database;
use crate::models::{Hotel, HotelWithPrice, PriceSnapshot, Room, SearchFilters, SortBy};
use anyhow::Result;
use rusqlite::params;

impl Database {
    pub fn upsert_hotel(&self, hotel: &Hotel) -> Result<()> {
        self.conn.execute(
            "INSERT INTO hotels (id, name, name_en, star, rating, rating_count, address, latitude, longitude, image_url, amenities, city, district, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, star=excluded.star, rating=excluded.rating,
                rating_count=excluded.rating_count, address=excluded.address,
                image_url=excluded.image_url, amenities=excluded.amenities,
                updated_at=excluded.updated_at",
            params![
                hotel.id,
                hotel.name,
                hotel.name_en,
                hotel.star,
                hotel.rating,
                hotel.rating_count,
                hotel.address,
                hotel.latitude,
                hotel.longitude,
                hotel.image_url,
                serde_json::to_string(&hotel.amenities)?,
                hotel.city,
                hotel.district,
                hotel.created_at,
                hotel.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_room(&self, room: &Room) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO rooms (id, hotel_id, name, bed_type, max_guests, area, has_window, has_breakfast, cancellation_policy)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                room.id,
                room.hotel_id,
                room.name,
                room.bed_type,
                room.max_guests,
                room.area,
                room.has_window,
                room.has_breakfast,
                room.cancellation_policy,
            ],
        )?;
        Ok(())
    }

    pub fn insert_price(&self, price: &PriceSnapshot) -> Result<()> {
        self.conn.execute(
            "INSERT INTO price_snapshots (id, room_id, hotel_id, price, original_price, checkin, checkout, scraped_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                price.id,
                price.room_id,
                price.hotel_id,
                price.price,
                price.original_price,
                price.checkin,
                price.checkout,
                price.scraped_at,
                price.source,
            ],
        )?;
        Ok(())
    }

    pub fn search_hotels(&self, filters: &SearchFilters) -> Result<Vec<HotelWithPrice>> {
        let mut sql = String::from(
            "SELECT h.*, MIN(p.price) as lowest_price, p.original_price, r.name as room_name
             FROM hotels h
             LEFT JOIN price_snapshots p ON h.id = p.hotel_id
             LEFT JOIN rooms r ON p.room_id = r.id
             WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(city) = &filters.city {
            sql.push_str(" AND h.city = ?");
            param_values.push(Box::new(city.clone()));
        }
        if let Some(min_star) = filters.min_star {
            sql.push_str(" AND h.star >= ?");
            param_values.push(Box::new(min_star));
        }
        if let Some(min_rating) = filters.min_rating {
            sql.push_str(" AND h.rating >= ?");
            param_values.push(Box::new(min_rating));
        }

        sql.push_str(" GROUP BY h.id");

        if let Some(min_price) = filters.min_price {
            sql.push_str(&format!(" HAVING lowest_price >= {min_price}"));
        }
        if let Some(max_price) = filters.max_price {
            if filters.min_price.is_some() {
                sql.push_str(&format!(" AND lowest_price <= {max_price}"));
            } else {
                sql.push_str(&format!(" HAVING lowest_price <= {max_price}"));
            }
        }

        match filters.sort_by {
            Some(SortBy::Price) => sql.push_str(" ORDER BY lowest_price ASC"),
            Some(SortBy::Rating) => sql.push_str(" ORDER BY h.rating DESC"),
            Some(SortBy::Star) => sql.push_str(" ORDER BY h.star DESC"),
            None => sql.push_str(" ORDER BY lowest_price ASC"),
        }

        if let Some(limit) = filters.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let amenities_str: String = row.get(10)?;
            let amenities: Vec<String> =
                serde_json::from_str(&amenities_str).unwrap_or_default();

            Ok(HotelWithPrice {
                hotel: Hotel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    name_en: row.get(2)?,
                    star: row.get(3)?,
                    rating: row.get(4)?,
                    rating_count: row.get::<_, Option<u32>>(5)?.unwrap_or(0),
                    address: row.get(6)?,
                    latitude: row.get(7)?,
                    longitude: row.get(8)?,
                    image_url: row.get(9)?,
                    amenities,
                    city: row.get(11)?,
                    district: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                },
                lowest_price: row.get(15)?,
                original_price: row.get(16)?,
                room_name: row.get(17)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_hotel(&self, id: &str) -> Result<Option<Hotel>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM hotels WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            let amenities_str: String = row.get(10)?;
            let amenities: Vec<String> =
                serde_json::from_str(&amenities_str).unwrap_or_default();
            Ok(Hotel {
                id: row.get(0)?,
                name: row.get(1)?,
                name_en: row.get(2)?,
                star: row.get(3)?,
                rating: row.get(4)?,
                rating_count: row.get::<_, Option<u32>>(5)?.unwrap_or(0),
                address: row.get(6)?,
                latitude: row.get(7)?,
                longitude: row.get(8)?,
                image_url: row.get(9)?,
                amenities,
                city: row.get(11)?,
                district: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_price_history(&self, hotel_id: &str) -> Result<Vec<PriceSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM price_snapshots WHERE hotel_id = ?1 ORDER BY scraped_at DESC",
        )?;
        let rows = stmt.query_map(params![hotel_id], |row| {
            Ok(PriceSnapshot {
                id: row.get(0)?,
                room_id: row.get(1)?,
                hotel_id: row.get(2)?,
                price: row.get(3)?,
                original_price: row.get(4)?,
                checkin: row.get(5)?,
                checkout: row.get(6)?,
                scraped_at: row.get(7)?,
                source: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn export_json(&self, filters: &SearchFilters) -> Result<String> {
        let hotels = self.search_hotels(filters)?;
        Ok(serde_json::to_string_pretty(&hotels)?)
    }

    pub fn export_csv(&self, filters: &SearchFilters) -> Result<String> {
        let hotels = self.search_hotels(filters)?;
        let mut csv = String::from("id,name,city,star,rating,price,room\n");
        for h in &hotels {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                h.hotel.id,
                h.hotel.name.replace(',', " "),
                h.hotel.city,
                h.hotel.star.map_or("-".to_string(), |s| s.to_string()),
                h.hotel.rating.map_or("-".to_string(), |r| format!("{r:.1}")),
                h.lowest_price.map_or("-".to_string(), |p| format!("{p:.0}")),
                h.room_name.as_deref().unwrap_or("-"),
            ));
        }
        Ok(csv)
    }
}
