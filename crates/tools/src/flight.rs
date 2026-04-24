use crate::cache::RedisCache;
use crate::scrape::{scrape_flights, ScrapedFlight};
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::{Database, Flight, FlightPrice};
use tracing::info;

#[derive(Deserialize)]
pub struct SearchFlightsParams {
    pub from_city: String,
    pub to_city: String,
    pub travel_date: String,
    pub cabin_class: Option<String>,
    pub max_price: Option<f64>,
    pub sort_by: Option<String>,
    pub limit: Option<usize>,
}

pub fn handle_search_flights(
    db: &Database,
    scraper_base_url: &str,
    redis: &RedisCache,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: SearchFlightsParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "search_flights".into(),
            message: format!("Invalid input: {e}"),
        })?;

    let _date = chrono::NaiveDate::parse_from_str(&params.travel_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "search_flights".into(),
            message: "出行日期格式错误，应为 YYYY-MM-DD".into(),
        })?;

    let limit = params.limit.unwrap_or(20).min(50);

    info!(
        "Searching flights: {} -> {} on {}",
        params.from_city, params.to_city, params.travel_date
    );

    // 1. Check Redis cache first
    if let Some(cached_json) = redis.get_transport("flight", &params.from_city, &params.to_city, &params.travel_date) {
        info!("Flight query served from Redis cache");
        return Ok(cached_json);
    }

    // 2. Check SQLite cache (60-minute window)
    let cached_results = db
        .search_flights(&params.from_city, &params.to_city, &params.travel_date, 60)
        .map_err(|e| RuntimeError::Tool {
            tool_name: "search_flights".into(),
            message: e.to_string(),
        })?;

    if !cached_results.is_empty() {
        let response = build_flight_response_from_cache(cached_results, &params, limit)?;
        redis.set_transport("flight", &params.from_city, &params.to_city, &params.travel_date, &response);
        return Ok(response);
    }

    // 3. Scrape from Python service
    let flights = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            scrape_flights(
                scraper_base_url,
                &params.from_city,
                &params.to_city,
                &params.travel_date,
            )
            .await
        })
    })
    .map_err(|e| RuntimeError::Tool {
        tool_name: "search_flights".into(),
        message: e.to_string(),
    })?;

    if flights.is_empty() {
        return Ok("未找到符合条件的航班。".to_string());
    }

    // 4. Persist scraped data
    persist_scraped_flights(db, &flights, &params.travel_date).map_err(|e| RuntimeError::Tool {
        tool_name: "search_flights".into(),
        message: format!("保存机票数据失败: {e}"),
    })?;

    // 5. Build response and cache in Redis
    let response = build_flight_response_from_scraped(flights, &params, limit)?;
    redis.set_transport("flight", &params.from_city, &params.to_city, &params.travel_date, &response);
    Ok(response)
}

fn build_flight_response_from_cache(
    cached_results: Vec<storage::FlightSearchResult>,
    params: &SearchFlightsParams,
    limit: usize,
) -> Result<String, RuntimeError> {
    let mut filtered = cached_results;

    if let Some(cabin) = &params.cabin_class {
        let cabin_cn = match cabin.as_str() {
            "economy" => "经济舱",
            "business" => "商务舱",
            "first" => "头等舱",
            _ => cabin.as_str(),
        };
        filtered.retain(|r| r.cabin_class.as_deref() == Some(cabin_cn));
    }

    if let Some(max_price) = params.max_price {
        filtered.retain(|r| r.lowest_price.map_or(true, |p| p <= max_price));
    }

    let sort_by = params.sort_by.as_deref().unwrap_or("price");
    match sort_by {
        "price" => filtered.sort_by(|a, b| {
            a.lowest_price
                .partial_cmp(&b.lowest_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "duration" => filtered.sort_by_key(|r| r.flight.duration_minutes),
        _ => filtered.sort_by(|a, b| a.flight.depart_time.cmp(&b.flight.depart_time)),
    }

    filtered.truncate(limit);

    let results: Vec<serde_json::Value> = filtered
        .iter()
        .map(|r| {
            serde_json::json!({
                "flight_id": r.flight.id,
                "airline": r.flight.airline,
                "from_airport": r.flight.from_airport,
                "to_airport": r.flight.to_airport,
                "depart_time": r.flight.depart_time,
                "arrive_time": r.flight.arrive_time,
                "duration": format!("{}小时{}分", r.flight.duration_minutes / 60, r.flight.duration_minutes % 60),
                "aircraft_type": r.flight.aircraft_type,
                "lowest_price": r.lowest_price,
                "prices": [{
                    "cabin": r.cabin_class,
                    "price": r.lowest_price,
                    "discount": r.discount,
                    "available": r.available_seats,
                }],
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": results.len(),
        "source": "cache",
        "flights": results
    }))
    .unwrap_or_default())
}

fn build_flight_response_from_scraped(
    flights: Vec<ScrapedFlight>,
    params: &SearchFlightsParams,
    limit: usize,
) -> Result<String, RuntimeError> {
    let mut filtered_flights = flights;

    if let Some(cabin) = &params.cabin_class {
        let cabin_cn = match cabin.as_str() {
            "economy" => "经济舱",
            "business" => "商务舱",
            "first" => "头等舱",
            _ => cabin.as_str(),
        };
        filtered_flights.retain(|f| f.prices.iter().any(|p| p.cabin_class == cabin_cn));
    }

    if let Some(max_price) = params.max_price {
        filtered_flights.retain(|f| f.prices.iter().any(|p| p.price <= max_price));
    }

    let sort_by = params.sort_by.as_deref().unwrap_or("price");
    match sort_by {
        "price" => {
            filtered_flights.sort_by(|a, b| {
                let a_price = a
                    .prices
                    .iter()
                    .map(|p| p.price)
                    .min_by(|x, y| x.partial_cmp(y).unwrap());
                let b_price = b
                    .prices
                    .iter()
                    .map(|p| p.price)
                    .min_by(|x, y| x.partial_cmp(y).unwrap());
                a_price
                    .partial_cmp(&b_price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        "duration" => {
            filtered_flights.sort_by_key(|f| f.duration_minutes);
        }
        _ => {
            filtered_flights.sort_by(|a, b| a.depart_time.cmp(&b.depart_time));
        }
    }

    filtered_flights.truncate(limit);

    let results: Vec<serde_json::Value> = filtered_flights
        .iter()
        .map(|f| {
            let lowest_price = f
                .prices
                .iter()
                .map(|p| p.price)
                .min_by(|x, y| x.partial_cmp(y).unwrap());
            serde_json::json!({
                "flight_id": f.flight_id,
                "airline": f.airline,
                "from_airport": f.from_airport,
                "to_airport": f.to_airport,
                "depart_time": f.depart_time,
                "arrive_time": f.arrive_time,
                "duration": format!("{}小时{}分", f.duration_minutes / 60, f.duration_minutes % 60),
                "aircraft_type": f.aircraft_type,
                "data_source": f.source,
                "lowest_price": lowest_price,
                "prices": f.prices.iter().map(|p| serde_json::json!({
                    "cabin": p.cabin_class,
                    "price": p.price,
                    "discount": p.discount,
                    "available": p.available_seats,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": results.len(),
        "source": "live",
        "flights": results
    }))
    .unwrap_or_default())
}

fn persist_scraped_flights(
    db: &Database,
    flights: &[ScrapedFlight],
    travel_date: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    for flight in flights {
        let flight_record = Flight {
            id: flight.flight_id.clone(),
            airline: flight.airline.clone(),
            from_airport: flight.from_airport.clone(),
            to_airport: flight.to_airport.clone(),
            from_city: flight.from_city.clone(),
            to_city: flight.to_city.clone(),
            depart_time: flight.depart_time.clone(),
            arrive_time: flight.arrive_time.clone(),
            duration_minutes: flight.duration_minutes,
            aircraft_type: flight.aircraft_type.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db.upsert_flight(&flight_record)?;

        for price in &flight.prices {
            let flight_price = FlightPrice {
                id: uuid::Uuid::new_v4().to_string(),
                flight_id: flight.flight_id.clone(),
                cabin_class: price.cabin_class.clone(),
                price: price.price,
                discount: price.discount,
                available_seats: price.available_seats,
                travel_date: travel_date.to_string(),
                scraped_at: now.clone(),
                source: flight.source.clone(),
            };
            db.insert_flight_price(&flight_price)?;
        }
    }

    Ok(())
}
