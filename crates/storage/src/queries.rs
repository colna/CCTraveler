use crate::db::Database;
use crate::models::{
    AirportCode, Attraction, City, District, Hotel, HotelWithPrice, PriceSnapshot, Room,
    SearchFilters, SortBy, StationCode,
};
use anyhow::Result;
use rusqlite::{params, OptionalExtension, Row};

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
            param_values.iter().map(std::convert::AsRef::as_ref).collect();

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

    pub fn resolve_city(&self, query: &str) -> Result<Option<City>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let normalized = normalize_city_name(trimmed);
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT c.id, c.name, c.name_en, c.province, c.latitude, c.longitude,
                    c.population, c.area_km2, c.tier, c.description, c.created_at
             FROM cities c
             LEFT JOIN city_mappings m ON m.city_id = c.id
             WHERE c.name = ?1
                OR c.name = ?2
                OR lower(COALESCE(c.name_en, '')) = lower(?1)
                OR lower(COALESCE(c.name_en, '')) = lower(?2)
                OR m.source_name = ?1
                OR m.source_name = ?2
                OR lower(COALESCE(m.pinyin, '')) = lower(?1)
                OR lower(COALESCE(m.pinyin, '')) = lower(?2)
                OR m.source_id = ?1
             ORDER BY CASE
                WHEN c.name = ?1 THEN 0
                WHEN c.name = ?2 THEN 1
                WHEN m.source_name = ?1 THEN 2
                WHEN m.source_name = ?2 THEN 3
                WHEN lower(COALESCE(m.pinyin, '')) = lower(?1) THEN 4
                WHEN lower(COALESCE(m.pinyin, '')) = lower(?2) THEN 5
                WHEN m.source_id = ?1 THEN 6
                ELSE 7
             END
             LIMIT 1",
        )?;

        stmt.query_row(params![trimmed, normalized], row_to_city)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_city_districts(&self, city_id: &str) -> Result<Vec<District>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, city_id, name, name_en, latitude, longitude, description, tags
             FROM districts
             WHERE city_id = ?1
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map(params![city_id], |row| {
            Ok(District {
                id: row.get(0)?,
                city_id: row.get(1)?,
                name: row.get(2)?,
                name_en: row.get(3)?,
                latitude: row.get(4)?,
                longitude: row.get(5)?,
                description: row.get(6)?,
                tags: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn list_city_attractions(&self, city_id: &str, category: Option<&str>) -> Result<Vec<Attraction>> {
        let mut results = Vec::new();

        if let Some(category) = category {
            let mut stmt = self.conn.prepare(
                "SELECT id, city_id, district_id, name, name_en, category, rating, latitude, longitude,
                        address, description, opening_hours, ticket_price, visit_duration_hours
                 FROM attractions
                 WHERE city_id = ?1 AND category = ?2
                 ORDER BY rating DESC, name ASC",
            )?;
            let rows = stmt.query_map(params![city_id, category], |row| {
                Ok(Attraction {
                    id: row.get(0)?,
                    city_id: row.get(1)?,
                    district_id: row.get(2)?,
                    name: row.get(3)?,
                    name_en: row.get(4)?,
                    category: row.get(5)?,
                    rating: row.get(6)?,
                    latitude: row.get(7)?,
                    longitude: row.get(8)?,
                    address: row.get(9)?,
                    description: row.get(10)?,
                    opening_hours: row.get(11)?,
                    ticket_price: row.get(12)?,
                    visit_duration_hours: row.get(13)?,
                })
            })?;

            for row in rows {
                results.push(row?);
            }
            return Ok(results);
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, city_id, district_id, name, name_en, category, rating, latitude, longitude,
                    address, description, opening_hours, ticket_price, visit_duration_hours
             FROM attractions
             WHERE city_id = ?1
             ORDER BY rating DESC, name ASC",
        )?;
        let rows = stmt.query_map(params![city_id], |row| {
            Ok(Attraction {
                id: row.get(0)?,
                city_id: row.get(1)?,
                district_id: row.get(2)?,
                name: row.get(3)?,
                name_en: row.get(4)?,
                category: row.get(5)?,
                rating: row.get(6)?,
                latitude: row.get(7)?,
                longitude: row.get(8)?,
                address: row.get(9)?,
                description: row.get(10)?,
                opening_hours: row.get(11)?,
                ticket_price: row.get(12)?,
                visit_duration_hours: row.get(13)?,
            })
        })?;

        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn list_city_station_codes(&self, city_name: &str) -> Result<Vec<StationCode>> {
        let mut stmt = self.conn.prepare(
            "SELECT city, station_name, station_code, created_at
             FROM station_codes
             WHERE city = ?1
             ORDER BY station_name ASC",
        )?;

        let rows = stmt.query_map(params![city_name], |row| {
            Ok(StationCode {
                city: row.get(0)?,
                station_name: row.get(1)?,
                station_code: row.get(2)?,
                created_at: row.get::<_, String>(3).ok(),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn list_city_airport_codes(&self, city_name: &str) -> Result<Vec<AirportCode>> {
        let mut stmt = self.conn.prepare(
            "SELECT city, airport_name, airport_code, iata_code, icao_code, created_at
             FROM airport_codes
             WHERE city = ?1
             ORDER BY airport_name ASC",
        )?;

        let rows = stmt.query_map(params![city_name], |row| {
            Ok(AirportCode {
                city: row.get(0)?,
                airport_name: row.get(1)?,
                airport_code: row.get(2)?,
                iata_code: row.get(3)?,
                icao_code: row.get(4)?,
                created_at: row.get::<_, String>(5).ok(),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

fn row_to_city(row: &Row<'_>) -> rusqlite::Result<City> {
    let province: String = row.get(3)?;
    Ok(City {
        id: row.get(0)?,
        name: row.get(1)?,
        name_en: row.get(2)?,
        province: if province.trim().is_empty() { None } else { Some(province) },
        latitude: row.get(4)?,
        longitude: row.get(5)?,
        population: row.get(6)?,
        area_km2: row.get(7)?,
        tier: row.get(8)?,
        description: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn normalize_city_name(name: &str) -> String {
    let mut normalized = name.trim().to_string();
    for (start, end) in [("(", ")"), ("（", "）")] {
        if let Some(index) = normalized.rfind(start) {
            if normalized.ends_with(end) {
                normalized.truncate(index);
                normalized = normalized.trim().to_string();
            }
        }
    }
    normalized
}
