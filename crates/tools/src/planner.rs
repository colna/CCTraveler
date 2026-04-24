use crate::scrape::{scrape_flights, scrape_trains};
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
pub struct PlanTripParams {
    pub from_city: String,
    pub to_city: String,
    pub start_date: String,
    pub end_date: String,
    pub budget: f64,
    pub travelers: Option<i32>,
    pub transport_priority: Option<String>,
    pub hotel_star: Option<u8>,
    pub interests: Option<Vec<String>>,
}

pub fn handle_plan_trip(
    db: &Database,
    scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: PlanTripParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "plan_trip".into(),
            message: format!("Invalid input: {e}"),
        })?;

    let start = chrono::NaiveDate::parse_from_str(&params.start_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "plan_trip".into(),
            message: "出发日期格式错误，应为 YYYY-MM-DD".into(),
        })?;
    let end = chrono::NaiveDate::parse_from_str(&params.end_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "plan_trip".into(),
            message: "返回日期格式错误，应为 YYYY-MM-DD".into(),
        })?;

    if end <= start {
        return Err(RuntimeError::Tool {
            tool_name: "plan_trip".into(),
            message: "返回日期必须晚于出发日期".into(),
        });
    }

    let days = (end - start).num_days();
    if days > 30 {
        return Err(RuntimeError::Tool {
            tool_name: "plan_trip".into(),
            message: "行程不能超过 30 天".into(),
        });
    }

    let travelers = params.travelers.unwrap_or(1).max(1);

    info!(
        "Planning trip: {} -> {} ({} to {}, {} days, budget ¥{:.0})",
        params.from_city, params.to_city, params.start_date, params.end_date, days, params.budget
    );

    // 1. Find transport options
    let transport = find_best_transport(db, scraper_base_url, &params);
    let transport_cost = transport
        .as_ref()
        .map(|t| t.2 * travelers as f64)
        .unwrap_or(0.0);

    // 2. Calculate remaining budget
    let remaining = (params.budget - transport_cost * 2.0).max(0.0); // round trip
    let hotel_budget_per_night = remaining * 0.6 / days as f64;
    let food_budget_per_day = remaining * 0.25 / days as f64;
    let activity_budget_per_day = remaining * 0.15 / days as f64;

    // 3. Find hotels from cache
    let hotels = find_hotels(db, &params.to_city, hotel_budget_per_night, params.hotel_star);

    // 4. Find attractions
    let attractions = find_attractions(db, &params.to_city, params.interests.as_deref());

    // 5. Generate daily plans
    let mut daily_plans = Vec::new();
    for day in 0..days {
        let date = start + chrono::Duration::days(day);
        let day_attractions: Vec<&serde_json::Value> = attractions
            .iter()
            .skip(day as usize * 2)
            .take(2)
            .collect();

        let activities: Vec<serde_json::Value> = day_attractions
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a["name"],
                    "category": a["category"],
                    "ticket_price": a["ticket_price"],
                    "visit_hours": a["visit_duration"],
                })
            })
            .collect();

        daily_plans.push(serde_json::json!({
            "day": day + 1,
            "date": date.format("%Y-%m-%d").to_string(),
            "theme": if day == 0 {
                "抵达 & 初探"
            } else if day == days - 1 {
                "返程"
            } else {
                "深度游览"
            },
            "activities": activities,
            "meals_budget": format!("¥{:.0}", food_budget_per_day),
        }));
    }

    // 6. Build the trip plan
    let transport_section = if let Some((mode, desc, price)) = &transport {
        serde_json::json!({
            "mode": mode,
            "description": desc,
            "price_per_person": price,
            "total_cost": *price * travelers as f64 * 2.0,
            "note": "往返费用"
        })
    } else {
        serde_json::json!({
            "mode": "未知",
            "description": "未找到交通方案",
            "price_per_person": 0,
            "total_cost": 0,
        })
    };

    let hotel_section = if !hotels.is_empty() {
        let h = &hotels[0];
        serde_json::json!({
            "recommendation": h["name"],
            "star": h["star"],
            "price_per_night": h["price"],
            "total_nights": days,
            "total_cost": h["price"].as_f64().unwrap_or(0.0) * days as f64,
        })
    } else {
        serde_json::json!({
            "recommendation": format!("建议预算 ¥{:.0}/晚 的酒店", hotel_budget_per_night),
            "price_per_night": hotel_budget_per_night,
            "total_nights": days,
            "total_cost": hotel_budget_per_night * days as f64,
        })
    };

    let budget_breakdown = serde_json::json!({
        "transport": transport_cost * 2.0,
        "hotel": hotel_budget_per_night * days as f64,
        "food": food_budget_per_day * days as f64,
        "activities": activity_budget_per_day * days as f64,
        "total_budget": params.budget,
        "estimated_total": transport_cost * 2.0 + hotel_budget_per_night * days as f64
            + food_budget_per_day * days as f64 + activity_budget_per_day * days as f64,
    });

    let plan = serde_json::json!({
        "trip": {
            "from": params.from_city,
            "to": params.to_city,
            "start_date": params.start_date,
            "end_date": params.end_date,
            "days": days,
            "travelers": travelers,
        },
        "transport": transport_section,
        "hotel": hotel_section,
        "daily_plans": daily_plans,
        "budget_breakdown": budget_breakdown,
    });

    Ok(serde_json::to_string_pretty(&plan).unwrap_or_default())
}

/// Find the best transport option (cheapest train or flight).
fn find_best_transport(
    db: &Database,
    scraper_base_url: &str,
    params: &PlanTripParams,
) -> Option<(String, String, f64)> {
    let priority = params.transport_priority.as_deref().unwrap_or("cost");

    // Get train option
    let train_opt = get_train_option(db, scraper_base_url, &params.from_city, &params.to_city, &params.start_date);
    // Get flight option
    let flight_opt = get_flight_option(db, scraper_base_url, &params.from_city, &params.to_city, &params.start_date);

    match (train_opt, flight_opt) {
        (Some(t), Some(f)) => {
            match priority {
                "time" => {
                    if t.2 <= f.2 { Some(t) } else { Some(f) } // compare by duration stored differently; use cost as proxy
                }
                "comfort" => Some(t), // trains are generally more comfortable
                _ => {
                    // cost: pick cheaper
                    if t.2 <= f.2 { Some(t) } else { Some(f) }
                }
            }
        }
        (Some(t), None) => Some(t),
        (None, Some(f)) => Some(f),
        (None, None) => None,
    }
}

fn get_train_option(
    db: &Database,
    scraper_base_url: &str,
    from: &str,
    to: &str,
    date: &str,
) -> Option<(String, String, f64)> {
    // Cache first
    if let Ok(results) = db.search_trains(from, to, date, 60) {
        if let Some(best) = results.into_iter().min_by(|a, b| {
            a.lowest_price.partial_cmp(&b.lowest_price).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let price = best.lowest_price.unwrap_or(0.0);
            let desc = format!("{} {} {}→{} ¥{:.0}",
                best.train.id, best.train.depart_time,
                best.train.from_station, best.train.to_station, price);
            return Some(("火车".to_string(), desc, price));
        }
    }

    // Live
    let trains = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            scrape_trains(scraper_base_url, from, to, date).await
        })
    }).ok()?;

    let best = trains.into_iter().min_by(|a, b| {
        let ap = a.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        let bp = b.seats.iter().map(|s| s.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let price = best.seats.iter().map(|s| s.price).min_by(|a, b| a.partial_cmp(b).unwrap())?;
    let desc = format!("{} {} {}→{} ¥{:.0}",
        best.train_id, best.depart_time, best.from_station, best.to_station, price);
    Some(("火车".to_string(), desc, price))
}

fn get_flight_option(
    db: &Database,
    scraper_base_url: &str,
    from: &str,
    to: &str,
    date: &str,
) -> Option<(String, String, f64)> {
    // Cache first
    if let Ok(results) = db.search_flights(from, to, date, 60) {
        if let Some(best) = results.into_iter().min_by(|a, b| {
            a.lowest_price.partial_cmp(&b.lowest_price).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let price = best.lowest_price.unwrap_or(0.0);
            let desc = format!("{} {} {} {}→{} ¥{:.0}",
                best.flight.id, best.flight.airline, best.flight.depart_time,
                best.flight.from_airport, best.flight.to_airport, price);
            return Some(("飞机".to_string(), desc, price));
        }
    }

    // Live
    let flights = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            scrape_flights(scraper_base_url, from, to, date).await
        })
    }).ok()?;

    let best = flights.into_iter().min_by(|a, b| {
        let ap = a.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        let bp = b.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
        ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let price = best.prices.iter().map(|p| p.price).min_by(|a, b| a.partial_cmp(b).unwrap())?;
    let desc = format!("{} {} {} {}→{} ¥{:.0}",
        best.flight_id, best.airline, best.depart_time, best.from_airport, best.to_airport, price);
    Some(("飞机".to_string(), desc, price))
}

fn find_hotels(
    db: &Database,
    city: &str,
    max_price: f64,
    min_star: Option<u8>,
) -> Vec<serde_json::Value> {
    let filters = storage::SearchFilters {
        city: Some(city.to_string()),
        max_price: Some(max_price),
        min_star,
        sort_by: Some(storage::SortBy::Rating),
        limit: Some(3),
        ..Default::default()
    };

    db.search_hotels(&filters)
        .unwrap_or_default()
        .into_iter()
        .map(|h| {
            serde_json::json!({
                "name": h.hotel.name,
                "star": h.hotel.star,
                "rating": h.hotel.rating,
                "price": h.lowest_price,
                "address": h.hotel.address,
            })
        })
        .collect()
}

fn find_attractions(
    db: &Database,
    city: &str,
    interests: Option<&[String]>,
) -> Vec<serde_json::Value> {
    let resolved = db.resolve_city(city).ok().flatten();
    let city_id = match &resolved {
        Some(c) => c.id.as_str(),
        None => return Vec::new(),
    };

    let mut all = Vec::new();

    if let Some(interests) = interests {
        for interest in interests {
            if let Ok(attractions) = db.list_city_attractions(city_id, Some(interest)) {
                for a in attractions {
                    all.push(serde_json::json!({
                        "name": a.name,
                        "category": a.category,
                        "rating": a.rating,
                        "ticket_price": a.ticket_price,
                        "visit_duration": a.visit_duration_hours,
                        "description": a.description,
                    }));
                }
            }
        }
    }

    if all.is_empty() {
        if let Ok(attractions) = db.list_city_attractions(city_id, None) {
            for a in attractions {
                all.push(serde_json::json!({
                    "name": a.name,
                    "category": a.category,
                    "rating": a.rating,
                    "ticket_price": a.ticket_price,
                    "visit_duration": a.visit_duration_hours,
                    "description": a.description,
                }));
            }
        }
    }

    all
}
