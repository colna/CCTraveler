use crate::cache::RedisCache;
use crate::scrape::{scrape_trains, ScrapedTrain};
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::{Database, Train, TrainPrice};
use tracing::info;

#[derive(Deserialize)]
pub struct SearchTrainsParams {
    pub from_city: String,
    pub to_city: String,
    pub travel_date: String,
    pub train_types: Option<Vec<String>>,
    pub sort_by: Option<String>,
    pub limit: Option<usize>,
}

pub fn handle_search_trains(
    db: &Database,
    scraper_base_url: &str,
    redis: &RedisCache,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: SearchTrainsParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "search_trains".into(),
            message: format!("Invalid input: {e}"),
        })?;

    let _date = chrono::NaiveDate::parse_from_str(&params.travel_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "search_trains".into(),
            message: "出行日期格式错误，应为 YYYY-MM-DD".into(),
        })?;

    let limit = params.limit.unwrap_or(20).min(50);

    info!(
        "Searching trains: {} -> {} on {}",
        params.from_city, params.to_city, params.travel_date
    );

    // 1. Check Redis cache first
    if let Some(cached_json) = redis.get_transport("train", &params.from_city, &params.to_city, &params.travel_date) {
        info!("Train query served from Redis cache");
        return Ok(cached_json);
    }

    // 2. Check SQLite cache
    let cached_results = db
        .search_trains(&params.from_city, &params.to_city, &params.travel_date, 60)
        .map_err(|e| RuntimeError::Tool {
            tool_name: "search_trains".into(),
            message: e.to_string(),
        })?;

    if !cached_results.is_empty() {
        let response = build_train_response_from_cache(cached_results, &params, limit)?;
        redis.set_transport("train", &params.from_city, &params.to_city, &params.travel_date, &response);
        return Ok(response);
    }

    // 3. Scrape from Python service
    let trains = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            scrape_trains(
                scraper_base_url,
                &params.from_city,
                &params.to_city,
                &params.travel_date,
            )
            .await
        })
    })
    .map_err(|e| RuntimeError::Tool {
        tool_name: "search_trains".into(),
        message: e.to_string(),
    })?;

    if trains.is_empty() {
        return Ok("未找到符合条件的火车票。".to_string());
    }

    persist_scraped_trains(db, &trains, &params.travel_date).map_err(|e| RuntimeError::Tool {
        tool_name: "search_trains".into(),
        message: format!("保存火车票数据失败: {e}"),
    })?;

    let response = build_train_response_from_scraped(trains, &params, limit)?;
    redis.set_transport("train", &params.from_city, &params.to_city, &params.travel_date, &response);
    Ok(response)
}

fn build_train_response_from_cache(
    cached_results: Vec<storage::TrainSearchResult>,
    params: &SearchTrainsParams,
    limit: usize,
) -> Result<String, RuntimeError> {
    let mut filtered_results = cached_results;

    if let Some(train_types) = &params.train_types {
        filtered_results.retain(|result| train_types.contains(&result.train.train_type));
    }

    let sort_by = params.sort_by.as_deref().unwrap_or("time");
    match sort_by {
        "price" => filtered_results.sort_by(|a, b| {
            a.lowest_price
                .partial_cmp(&b.lowest_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "duration" => filtered_results.sort_by_key(|result| result.train.duration_minutes),
        _ => filtered_results.sort_by(|a, b| a.train.depart_time.cmp(&b.train.depart_time)),
    }

    filtered_results.truncate(limit);

    let results: Vec<serde_json::Value> = filtered_results
        .iter()
        .map(|result| {
            serde_json::json!({
                "train_id": result.train.id,
                "train_type": result.train.train_type,
                "from_station": result.train.from_station,
                "to_station": result.train.to_station,
                "depart_time": result.train.depart_time,
                "arrive_time": result.train.arrive_time,
                "duration": format!("{}小时{}分", result.train.duration_minutes / 60, result.train.duration_minutes % 60),
                "lowest_price": result.lowest_price,
                "seats": [{
                    "type": result.seat_type,
                    "price": result.lowest_price,
                    "available": result.available_seats,
                }],
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": results.len(),
        "source": "cache",
        "trains": results
    }))
    .unwrap_or_default())
}

fn build_train_response_from_scraped(
    trains: Vec<ScrapedTrain>,
    params: &SearchTrainsParams,
    limit: usize,
) -> Result<String, RuntimeError> {
    let mut filtered_trains = trains;
    if let Some(train_types) = &params.train_types {
        filtered_trains.retain(|t| train_types.contains(&t.train_type));
    }

    let sort_by = params.sort_by.as_deref().unwrap_or("time");
    match sort_by {
        "price" => {
            filtered_trains.sort_by(|a, b| {
                let a_price = a.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
                let b_price = b.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
                a_price.partial_cmp(&b_price).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        "duration" => {
            filtered_trains.sort_by_key(|t| t.duration_minutes);
        }
        _ => {
            filtered_trains.sort_by(|a, b| a.depart_time.cmp(&b.depart_time));
        }
    }

    filtered_trains.truncate(limit);

    let results: Vec<serde_json::Value> = filtered_trains
        .iter()
        .map(|t| {
            let lowest_price = t.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
            serde_json::json!({
                "train_id": t.train_id,
                "train_type": t.train_type,
                "from_station": t.from_station,
                "to_station": t.to_station,
                "depart_time": t.depart_time,
                "arrive_time": t.arrive_time,
                "duration": format!("{}小时{}分", t.duration_minutes / 60, t.duration_minutes % 60),
                "lowest_price": lowest_price,
                "seats": t.seats.iter().map(|s| serde_json::json!({
                    "type": s.seat_type,
                    "price": s.price,
                    "available": s.available_seats,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": results.len(),
        "source": "live",
        "trains": results
    }))
    .unwrap_or_default())
}

fn persist_scraped_trains(
    db: &Database,
    trains: &[ScrapedTrain],
    travel_date: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    for train in trains {
        let train_record = Train {
            id: train.train_id.clone(),
            train_type: train.train_type.clone(),
            from_station: train.from_station.clone(),
            to_station: train.to_station.clone(),
            from_city: train.from_city.clone(),
            to_city: train.to_city.clone(),
            depart_time: train.depart_time.clone(),
            arrive_time: train.arrive_time.clone(),
            duration_minutes: train.duration_minutes,
            distance_km: train.distance_km,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db.upsert_train(&train_record)?;

        for seat in &train.seats {
            let train_price = TrainPrice {
                id: uuid::Uuid::new_v4().to_string(),
                train_id: train.train_id.clone(),
                seat_type: seat.seat_type.clone(),
                price: seat.price,
                available_seats: seat.available_seats,
                travel_date: travel_date.to_string(),
                scraped_at: now.clone(),
                source: "12306".to_string(),
            };
            db.insert_train_price(&train_price)?;
        }
    }

    Ok(())
}
