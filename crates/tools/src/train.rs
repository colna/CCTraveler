use crate::scrape::scrape_trains;
use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
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
    _db: &Database,
    scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: SearchTrainsParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "search_trains".into(),
            message: format!("Invalid input: {e}"),
        })?;

    // 验证日期格式
    let _date = chrono::NaiveDate::parse_from_str(&params.travel_date, "%Y-%m-%d")
        .map_err(|_| RuntimeError::Tool {
            tool_name: "search_trains".into(),
            message: "出行日期格式错误，应为 YYYY-MM-DD".into(),
        })?;

    // 限制返回数量
    let limit = params.limit.unwrap_or(20).min(50);

    info!(
        "Searching trains: {} -> {} on {}",
        params.from_city, params.to_city, params.travel_date
    );

    // 1. 先查询本地数据库
    // TODO: 实现数据库查询逻辑
    // let results = db.search_trains(...)?;

    // 2. 如果本地没有数据，调用爬虫服务
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

    // 3. 筛选车型
    let mut filtered_trains = trains;
    if let Some(train_types) = &params.train_types {
        filtered_trains.retain(|t| train_types.contains(&t.train_type));
    }

    // 4. 排序
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
            // 默认按时间排序（出发时间）
            filtered_trains.sort_by(|a, b| a.depart_time.cmp(&b.depart_time));
        }
    }

    // 5. 限制数量
    filtered_trains.truncate(limit);

    // 6. 构建响应
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
        "trains": results
    }))
    .unwrap_or_default())
}
