use crate::db::Database;
use crate::models::{
    AirportCode, Attraction, City, District, Flight, FlightPrice, FlightSearchResult,
    Hotel, HotelWithPrice, PriceSnapshot, Room,
    SearchFilters, SortBy, StationCode, Train, TrainPrice, TrainSearchResult,
    WikiEntry,
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

    pub fn upsert_train(&self, train: &Train) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trains (id, train_type, from_station, to_station, from_city, to_city, depart_time, arrive_time, duration_minutes, distance_km, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                train_type = excluded.train_type,
                from_station = excluded.from_station,
                to_station = excluded.to_station,
                from_city = excluded.from_city,
                to_city = excluded.to_city,
                depart_time = excluded.depart_time,
                arrive_time = excluded.arrive_time,
                duration_minutes = excluded.duration_minutes,
                distance_km = excluded.distance_km,
                updated_at = excluded.updated_at",
            params![
                train.id,
                train.train_type,
                train.from_station,
                train.to_station,
                train.from_city,
                train.to_city,
                train.depart_time,
                train.arrive_time,
                train.duration_minutes,
                train.distance_km,
                train.created_at,
                train.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_train_price(&self, price: &TrainPrice) -> Result<()> {
        self.conn.execute(
            "INSERT INTO train_prices (id, train_id, seat_type, price, available_seats, travel_date, scraped_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                price.id,
                price.train_id,
                price.seat_type,
                price.price,
                price.available_seats,
                price.travel_date,
                price.scraped_at,
                price.source,
            ],
        )?;
        Ok(())
    }

    pub fn search_trains(
        &self,
        from_city: &str,
        to_city: &str,
        travel_date: &str,
        max_age_minutes: i64,
    ) -> Result<Vec<TrainSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.train_type, t.from_station, t.to_station, t.from_city, t.to_city,
                    t.depart_time, t.arrive_time, t.duration_minutes, t.distance_km, t.created_at,
                    t.updated_at, tp.price, tp.seat_type, tp.available_seats
             FROM trains t
             JOIN (
                 SELECT tp1.train_id, tp1.price, tp1.seat_type, tp1.available_seats
                 FROM train_prices tp1
                 JOIN (
                     SELECT train_id, MAX(scraped_at) AS latest_scraped_at
                     FROM train_prices
                     WHERE travel_date = ?3
                       AND scraped_at >= datetime('now', '-' || ?4 || ' minutes')
                     GROUP BY train_id
                 ) latest
                   ON latest.train_id = tp1.train_id AND latest.latest_scraped_at = tp1.scraped_at
                 WHERE tp1.travel_date = ?3
                   AND tp1.price = (
                       SELECT MIN(tp2.price)
                       FROM train_prices tp2
                       WHERE tp2.train_id = tp1.train_id
                         AND tp2.travel_date = tp1.travel_date
                         AND tp2.scraped_at = tp1.scraped_at
                   )
                 LIMIT -1
             ) tp ON tp.train_id = t.id
             WHERE t.from_city = ?1 AND t.to_city = ?2
             ORDER BY t.depart_time ASC",
        )?;

        let rows = stmt.query_map(params![from_city, to_city, travel_date, max_age_minutes], |row| {
            Ok(TrainSearchResult {
                train: Train {
                    id: row.get(0)?,
                    train_type: row.get(1)?,
                    from_station: row.get(2)?,
                    to_station: row.get(3)?,
                    from_city: row.get(4)?,
                    to_city: row.get(5)?,
                    depart_time: row.get(6)?,
                    arrive_time: row.get(7)?,
                    duration_minutes: row.get(8)?,
                    distance_km: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                },
                lowest_price: row.get(12)?,
                seat_type: row.get(13)?,
                available_seats: row.get(14)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn upsert_flight(&self, flight: &Flight) -> Result<()> {
        self.conn.execute(
            "INSERT INTO flights (id, airline, from_airport, to_airport, from_city, to_city, depart_time, arrive_time, duration_minutes, aircraft_type, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                airline = excluded.airline,
                from_airport = excluded.from_airport,
                to_airport = excluded.to_airport,
                from_city = excluded.from_city,
                to_city = excluded.to_city,
                depart_time = excluded.depart_time,
                arrive_time = excluded.arrive_time,
                duration_minutes = excluded.duration_minutes,
                aircraft_type = excluded.aircraft_type,
                updated_at = excluded.updated_at",
            params![
                flight.id,
                flight.airline,
                flight.from_airport,
                flight.to_airport,
                flight.from_city,
                flight.to_city,
                flight.depart_time,
                flight.arrive_time,
                flight.duration_minutes,
                flight.aircraft_type,
                flight.created_at,
                flight.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_flight_price(&self, price: &FlightPrice) -> Result<()> {
        self.conn.execute(
            "INSERT INTO flight_prices (id, flight_id, cabin_class, price, discount, available_seats, travel_date, scraped_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                price.id,
                price.flight_id,
                price.cabin_class,
                price.price,
                price.discount,
                price.available_seats,
                price.travel_date,
                price.scraped_at,
                price.source,
            ],
        )?;
        Ok(())
    }

    pub fn search_flights(
        &self,
        from_city: &str,
        to_city: &str,
        travel_date: &str,
        max_age_minutes: i64,
    ) -> Result<Vec<FlightSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.airline, f.from_airport, f.to_airport, f.from_city, f.to_city,
                    f.depart_time, f.arrive_time, f.duration_minutes, f.aircraft_type,
                    f.created_at, f.updated_at,
                    fp.price, fp.cabin_class, fp.discount, fp.available_seats
             FROM flights f
             JOIN (
                 SELECT fp1.flight_id, fp1.price, fp1.cabin_class, fp1.discount, fp1.available_seats
                 FROM flight_prices fp1
                 JOIN (
                     SELECT flight_id, MAX(scraped_at) AS latest_scraped_at
                     FROM flight_prices
                     WHERE travel_date = ?3
                       AND scraped_at >= datetime('now', '-' || ?4 || ' minutes')
                     GROUP BY flight_id
                 ) latest
                   ON latest.flight_id = fp1.flight_id AND latest.latest_scraped_at = fp1.scraped_at
                 WHERE fp1.travel_date = ?3
                   AND fp1.price = (
                       SELECT MIN(fp2.price)
                       FROM flight_prices fp2
                       WHERE fp2.flight_id = fp1.flight_id
                         AND fp2.travel_date = fp1.travel_date
                         AND fp2.scraped_at = fp1.scraped_at
                   )
                 LIMIT -1
             ) fp ON fp.flight_id = f.id
             WHERE f.from_city = ?1 AND f.to_city = ?2
             ORDER BY f.depart_time ASC",
        )?;

        let rows = stmt.query_map(
            params![from_city, to_city, travel_date, max_age_minutes],
            |row| {
                Ok(FlightSearchResult {
                    flight: Flight {
                        id: row.get(0)?,
                        airline: row.get(1)?,
                        from_airport: row.get(2)?,
                        to_airport: row.get(3)?,
                        from_city: row.get(4)?,
                        to_city: row.get(5)?,
                        depart_time: row.get(6)?,
                        arrive_time: row.get(7)?,
                        duration_minutes: row.get(8)?,
                        aircraft_type: row.get(9)?,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    },
                    lowest_price: row.get(12)?,
                    cabin_class: row.get(13)?,
                    discount: row.get(14)?,
                    available_seats: row.get(15)?,
                })
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
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

    /// List all cities that have lat/lng coordinates.
    pub fn list_cities_with_coords(&self) -> Result<Vec<City>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, name_en, province, latitude, longitude, population, area_km2, tier, description, created_at
             FROM cities
             WHERE latitude IS NOT NULL AND longitude IS NOT NULL
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map([], row_to_city)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Insert or update a price subscription.
    pub fn upsert_price_subscription(
        &self,
        id: &str,
        user_id: &str,
        from_city: &str,
        to_city: &str,
        transport_type: &str,
        threshold: f64,
        created_at: &str,
        expires_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO price_subscriptions (id, user_id, from_city, to_city, transport_type, threshold, is_active, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                threshold = excluded.threshold,
                is_active = 1,
                expires_at = excluded.expires_at",
            params![id, user_id, from_city, to_city, transport_type, threshold, created_at, expires_at],
        )?;
        Ok(())
    }

    /// List active price subscriptions.
    pub fn list_active_subscriptions(&self, user_id: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let (sql, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(uid) = user_id {
            (
                "SELECT id, user_id, from_city, to_city, transport_type, threshold, created_at, expires_at
                 FROM price_subscriptions WHERE is_active = 1 AND user_id = ?1 ORDER BY created_at DESC".to_string(),
                vec![Box::new(uid.to_string())],
            )
        } else {
            (
                "SELECT id, user_id, from_city, to_city, transport_type, threshold, created_at, expires_at
                 FROM price_subscriptions WHERE is_active = 1 ORDER BY created_at DESC".to_string(),
                vec![],
            )
        };

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "user_id": row.get::<_, String>(1)?,
                "from_city": row.get::<_, String>(2)?,
                "to_city": row.get::<_, String>(3)?,
                "transport_type": row.get::<_, String>(4)?,
                "threshold": row.get::<_, f64>(5)?,
                "created_at": row.get::<_, String>(6)?,
                "expires_at": row.get::<_, String>(7)?,
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Deactivate a price subscription.
    pub fn deactivate_subscription(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE price_subscriptions SET is_active = 0 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
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

    // ============================================================
    // Wiki queries
    // ============================================================

    /// Insert or update a wiki entry (upsert by topic+key).
    pub fn upsert_wiki_entry(
        &self,
        id: &str,
        topic: &str,
        key: &str,
        value: &str,
        metadata: Option<&str>,
        now: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO wiki_entries (id, topic, key, value, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(topic, key) DO UPDATE SET
                value = excluded.value,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at",
            params![id, topic, key, value, metadata, now],
        )?;
        Ok(())
    }

    /// Get a single wiki entry by topic and key.
    pub fn get_wiki_entry(&self, topic: &str, key: &str) -> Result<Option<WikiEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, topic, key, value, metadata, created_at, updated_at
             FROM wiki_entries WHERE topic = ?1 AND key = ?2",
        )?;
        stmt.query_row(params![topic, key], |row| {
            Ok(WikiEntry {
                id: row.get(0)?,
                topic: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                metadata: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .optional()
        .map_err(Into::into)
    }

    /// List wiki entries, optionally filtered by topic.
    pub fn list_wiki_entries(&self, topic: Option<&str>) -> Result<Vec<WikiEntry>> {
        let mut results = Vec::new();

        if let Some(topic) = topic {
            let mut stmt = self.conn.prepare(
                "SELECT id, topic, key, value, metadata, created_at, updated_at
                 FROM wiki_entries WHERE topic = ?1 ORDER BY key ASC",
            )?;
            let rows = stmt.query_map(params![topic], row_to_wiki)?;
            for row in rows {
                results.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, topic, key, value, metadata, created_at, updated_at
                 FROM wiki_entries ORDER BY topic ASC, key ASC",
            )?;
            let rows = stmt.query_map([], row_to_wiki)?;
            for row in rows {
                results.push(row?);
            }
        }

        Ok(results)
    }

    /// Delete a wiki entry by topic and key.
    pub fn delete_wiki_entry(&self, topic: &str, key: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM wiki_entries WHERE topic = ?1 AND key = ?2",
            params![topic, key],
        )?;
        Ok(())
    }
}

fn row_to_wiki(row: &Row<'_>) -> rusqlite::Result<WikiEntry> {
    Ok(WikiEntry {
        id: row.get(0)?,
        topic: row.get(1)?,
        key: row.get(2)?,
        value: row.get(3)?,
        metadata: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
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
