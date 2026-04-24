use crate::scrape::{scrape_flights, scrape_trains};
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
pub struct CompareRoutesParams {
    pub from_city: String,
    pub to_city: String,
    pub travel_date: String,
    pub budget: Option<f64>,
    pub priority: Option<String>,
}

pub fn handle_compare_routes(
    db: &Database,
    scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: CompareRoutesParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "compare_routes".into(),
            message: format!("Invalid input: {e}"),
        })?;

    let _date = chrono::NaiveDate::parse_from_str(&params.travel_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "compare_routes".into(),
            message: "出行日期格式错误，应为 YYYY-MM-DD".into(),
        })?;

    info!(
        "Comparing routes: {} -> {} on {}, priority: {:?}",
        params.from_city, params.to_city, params.travel_date, params.priority
    );

    let priority = params.priority.as_deref().unwrap_or("cost");

    // 1. Fetch train data (cache first, then live)
    let train_results = get_train_data(db, scraper_base_url, &params);
    // 2. Fetch flight data (cache first, then live)
    let flight_results = get_flight_data(db, scraper_base_url, &params);

    // 3. Build route options
    let mut routes = Vec::new();

    if let Some((train_id, train_type, depart, arrive, duration, price, seat_type, from_st, to_st)) =
        train_results
    {
        let comfort_score = match train_type.as_str() {
            "G" => 9,
            "D" => 8,
            "C" => 8,
            "Z" => 7,
            "T" => 6,
            "K" => 5,
            _ => 5,
        };

        routes.push(serde_json::json!({
            "type": match train_type.as_str() {
                "G" => "高铁",
                "D" => "动车",
                "C" => "城际",
                _ => "火车",
            },
            "train_id": train_id,
            "time": {
                "depart": depart,
                "arrive": arrive,
                "total_minutes": duration,
                "description": format!("{}小时{}分", duration / 60, duration % 60)
            },
            "cost": {
                "ticket": price,
                "transport": 0,
                "total": price,
                "description": format!("¥{:.0}（{}）", price, seat_type)
            },
            "comfort": {
                "score": comfort_score,
                "description": match train_type.as_str() {
                    "G" => "舒适，市区直达，准点率高",
                    "D" => "较舒适，市区直达",
                    _ => "经济实惠，适合不赶时间的行程",
                }
            },
            "from_station": from_st,
            "to_station": to_st,
        }));
    }

    if let Some((flight_id, airline, depart, arrive, duration, price, cabin, from_ap, to_ap)) =
        flight_results
    {
        let airport_time = 120; // Estimated airport transit time
        let total_minutes = duration + airport_time;

        routes.push(serde_json::json!({
            "type": "飞机",
            "flight_id": flight_id,
            "airline": airline,
            "time": {
                "depart": depart,
                "arrive": arrive,
                "flight_minutes": duration,
                "airport_time": airport_time,
                "total_minutes": total_minutes,
                "description": format!("{}小时{}分飞行 + 2小时机场时间 = {}小时{}分",
                    duration / 60, duration % 60,
                    total_minutes / 60, total_minutes % 60)
            },
            "cost": {
                "ticket": price,
                "transport": 100,
                "total": price + 100.0,
                "description": format!("¥{:.0}（{}）+ ¥100（机场交通）", price, cabin)
            },
            "comfort": {
                "score": 7,
                "description": "较舒适，需提前到机场"
            },
            "from_airport": from_ap,
            "to_airport": to_ap,
        }));
    }

    if routes.is_empty() {
        return Ok("未找到任何交通方案。请确认城市名称和日期是否正确。".to_string());
    }

    // 4. Mark recommendation based on priority
    let recommended_idx = match priority {
        "time" => routes
            .iter()
            .enumerate()
            .min_by_key(|(_, r)| {
                r["time"]["total_minutes"].as_i64().unwrap_or(i64::MAX)
            })
            .map(|(i, _)| i),
        "cost" => routes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let a_cost = a["cost"]["total"].as_f64().unwrap_or(f64::MAX);
                let b_cost = b["cost"]["total"].as_f64().unwrap_or(f64::MAX);
                a_cost.partial_cmp(&b_cost).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i),
        "comfort" => routes
            .iter()
            .enumerate()
            .max_by_key(|(_, r)| r["comfort"]["score"].as_i64().unwrap_or(0))
            .map(|(i, _)| i),
        _ => routes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let a_cost = a["cost"]["total"].as_f64().unwrap_or(f64::MAX);
                let b_cost = b["cost"]["total"].as_f64().unwrap_or(f64::MAX);
                a_cost.partial_cmp(&b_cost).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i),
    };

    // Set recommended flag
    let mut route_values: Vec<serde_json::Value> = routes
        .into_iter()
        .enumerate()
        .map(|(i, mut r)| {
            r["recommended"] = serde_json::json!(Some(i) == recommended_idx);
            r
        })
        .collect();

    // Apply budget filter if specified
    if let Some(budget) = params.budget {
        route_values.retain(|r| {
            r["cost"]["total"].as_f64().unwrap_or(0.0) <= budget
        });
    }

    let recommended_type = recommended_idx
        .and_then(|_| route_values.iter().find(|r| r["recommended"].as_bool() == Some(true)))
        .and_then(|r| r["type"].as_str())
        .unwrap_or("未知");

    let reason = match priority {
        "time" => format!("{}总时间最短，适合赶时间的行程", recommended_type),
        "cost" => format!("{}性价比最高，费用最低", recommended_type),
        "comfort" => format!("{}舒适度最高", recommended_type),
        _ => format!("{}综合性价比最优", recommended_type),
    };

    let comparison = serde_json::json!({
        "routes": route_values,
        "recommendation": {
            "priority": priority,
            "choice": recommended_type,
            "reason": reason
        }
    });

    Ok(serde_json::to_string_pretty(&comparison).unwrap_or_default())
}

/// Fetch best train option: returns (train_id, type, depart, arrive, duration, price, seat_type, from_station, to_station)
fn get_train_data(
    db: &Database,
    scraper_base_url: &str,
    params: &CompareRoutesParams,
) -> Option<(String, String, String, String, i32, f64, String, String, String)> {
    // Try cache first
    if let Ok(cached) = db.search_trains(&params.from_city, &params.to_city, &params.travel_date, 60) {
        if let Some(best) = cached.into_iter().min_by(|a, b| {
            a.lowest_price
                .partial_cmp(&b.lowest_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            return Some((
                best.train.id,
                best.train.train_type,
                best.train.depart_time,
                best.train.arrive_time,
                best.train.duration_minutes,
                best.lowest_price.unwrap_or(0.0),
                best.seat_type.unwrap_or_else(|| "二等座".to_string()),
                best.train.from_station,
                best.train.to_station,
            ));
        }
    }

    // Try live scraping
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
    .ok()?;

    let best = trains.into_iter().min_by(|a, b| {
        let a_price = a.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        let b_price = b.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        a_price
            .partial_cmp(&b_price)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let cheapest_seat = best
        .seats
        .iter()
        .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal))?;

    Some((
        best.train_id,
        best.train_type,
        best.depart_time,
        best.arrive_time,
        best.duration_minutes,
        cheapest_seat.price,
        cheapest_seat.seat_type.clone(),
        best.from_station,
        best.to_station,
    ))
}

/// Fetch best flight option: returns (flight_id, airline, depart, arrive, duration, price, cabin, from_airport, to_airport)
fn get_flight_data(
    db: &Database,
    scraper_base_url: &str,
    params: &CompareRoutesParams,
) -> Option<(String, String, String, String, i32, f64, String, String, String)> {
    // Try cache first
    if let Ok(cached) = db.search_flights(&params.from_city, &params.to_city, &params.travel_date, 60) {
        if let Some(best) = cached.into_iter().min_by(|a, b| {
            a.lowest_price
                .partial_cmp(&b.lowest_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            return Some((
                best.flight.id,
                best.flight.airline,
                best.flight.depart_time,
                best.flight.arrive_time,
                best.flight.duration_minutes,
                best.lowest_price.unwrap_or(0.0),
                best.cabin_class.unwrap_or_else(|| "经济舱".to_string()),
                best.flight.from_airport,
                best.flight.to_airport,
            ));
        }
    }

    // Try live scraping
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
    .ok()?;

    let best = flights.into_iter().min_by(|a, b| {
        let a_price = a.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        let b_price = b.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        a_price
            .partial_cmp(&b_price)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let cheapest_price = best
        .prices
        .iter()
        .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal))?;

    Some((
        best.flight_id,
        best.airline,
        best.depart_time,
        best.arrive_time,
        best.duration_minutes,
        cheapest_price.price,
        cheapest_price.cabin_class.clone(),
        best.from_airport,
        best.to_airport,
    ))
}
