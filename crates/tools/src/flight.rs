use crate::scrape::scrape_flights;
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
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
    _db: &Database,
    scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: SearchFlightsParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "search_flights".into(),
            message: format!("Invalid input: {e}"),
        })?;

    // 验证日期格式
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

    // 调用爬虫服务
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

    let mut filtered_flights = flights;

    // 筛选舱位和价格
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
        filtered_flights.retain(|f| {
            f.prices.iter().any(|p| p.price <= max_price)
        });
    }

    // 排序
    let sort_by = params.sort_by.as_deref().unwrap_or("price");
    match sort_by {
        "price" => {
            filtered_flights.sort_by(|a, b| {
                let a_price = a.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
                let b_price = b.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
                a_price.partial_cmp(&b_price).unwrap_or(std::cmp::Ordering::Equal)
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

    // 构建响应
    let results: Vec<serde_json::Value> = filtered_flights
        .iter()
        .map(|f| {
            let lowest_price = f.prices.iter().map(|p| p.price).min_by(|x, y| x.partial_cmp(y).unwrap());
            serde_json::json!({
                "flight_id": f.flight_id,
                "airline": f.airline,
                "from_airport": f.from_airport,
                "to_airport": f.to_airport,
                "depart_time": f.depart_time,
                "arrive_time": f.arrive_time,
                "duration": format!("{}小时{}分", f.duration_minutes / 60, f.duration_minutes % 60),
                "aircraft_type": f.aircraft_type,
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
        "flights": results
    }))
    .unwrap_or_default())
}
