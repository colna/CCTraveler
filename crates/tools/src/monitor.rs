use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
pub struct PriceMonitorParams {
    pub action: String, // "subscribe", "list", "unsubscribe", "check"
    pub from_city: Option<String>,
    pub to_city: Option<String>,
    pub transport_type: Option<String>,
    pub threshold: Option<f64>,
    pub subscription_id: Option<String>,
    pub user_id: Option<String>,
}

pub fn handle_price_monitor(
    db: &Database,
    scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: PriceMonitorParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "price_monitor".into(),
            message: format!("Invalid input: {e}"),
        })?;

    info!("Price monitor action: {}", params.action);

    match params.action.as_str() {
        "subscribe" => handle_subscribe(db, &params),
        "list" => handle_list(db, &params),
        "unsubscribe" => handle_unsubscribe(db, &params),
        "check" => handle_check(db, scraper_base_url, &params),
        other => Err(RuntimeError::Tool {
            tool_name: "price_monitor".into(),
            message: format!("Unknown action: {}. Use subscribe/list/unsubscribe/check.", other),
        }),
    }
}

fn handle_subscribe(db: &Database, params: &PriceMonitorParams) -> Result<String, RuntimeError> {
    let from_city = params.from_city.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: "subscribe 需要 from_city".into(),
    })?;
    let to_city = params.to_city.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: "subscribe 需要 to_city".into(),
    })?;
    let transport_type = params.transport_type.as_deref().unwrap_or("train");
    let threshold = params.threshold.ok_or_else(|| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: "subscribe 需要 threshold（价格阈值）".into(),
    })?;
    let user_id = params.user_id.as_deref().unwrap_or("default");

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    db.upsert_price_subscription(&id, user_id, from_city, to_city, transport_type, threshold, &now, &expires_at)
        .map_err(|e| RuntimeError::Tool {
            tool_name: "price_monitor".into(),
            message: format!("保存订阅失败: {e}"),
        })?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "status": "subscribed",
        "id": id,
        "route": format!("{} → {}", from_city, to_city),
        "transport_type": transport_type,
        "threshold": threshold,
        "expires_at": expires_at,
        "message": format!("已订阅 {} → {} 的{}价格监控，当价格低于 ¥{:.0} 时会提醒您。有效期 7 天。",
            from_city, to_city,
            if transport_type == "train" { "火车票" } else { "机票" },
            threshold)
    }))
    .unwrap_or_default())
}

fn handle_list(db: &Database, params: &PriceMonitorParams) -> Result<String, RuntimeError> {
    let user_id = params.user_id.as_deref();

    let subs = db.list_active_subscriptions(user_id).map_err(|e| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: e.to_string(),
    })?;

    if subs.is_empty() {
        return Ok("当前没有活跃的价格订阅。".to_string());
    }

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": subs.len(),
        "subscriptions": subs
    }))
    .unwrap_or_default())
}

fn handle_unsubscribe(db: &Database, params: &PriceMonitorParams) -> Result<String, RuntimeError> {
    let id = params.subscription_id.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: "unsubscribe 需要 subscription_id".into(),
    })?;

    db.deactivate_subscription(id).map_err(|e| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: e.to_string(),
    })?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "status": "unsubscribed",
        "id": id,
        "message": "已取消价格订阅"
    }))
    .unwrap_or_default())
}

fn handle_check(
    db: &Database,
    scraper_base_url: &str,
    params: &PriceMonitorParams,
) -> Result<String, RuntimeError> {
    let subs = db.list_active_subscriptions(params.user_id.as_deref()).map_err(|e| RuntimeError::Tool {
        tool_name: "price_monitor".into(),
        message: e.to_string(),
    })?;

    if subs.is_empty() {
        return Ok("没有活跃的价格订阅需要检查。".to_string());
    }

    let mut alerts = Vec::new();
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    for sub in &subs {
        let from_city = sub["from_city"].as_str().unwrap_or("");
        let to_city = sub["to_city"].as_str().unwrap_or("");
        let transport_type = sub["transport_type"].as_str().unwrap_or("train");
        let threshold = sub["threshold"].as_f64().unwrap_or(0.0);

        let current_price = if transport_type == "train" {
            get_cheapest_train_price(db, scraper_base_url, from_city, to_city, &tomorrow)
        } else {
            get_cheapest_flight_price(db, scraper_base_url, from_city, to_city, &tomorrow)
        };

        if let Some(price) = current_price {
            let status = if price <= threshold { "triggered" } else { "watching" };
            alerts.push(serde_json::json!({
                "route": format!("{} → {}", from_city, to_city),
                "transport_type": transport_type,
                "threshold": threshold,
                "current_price": price,
                "status": status,
                "message": if price <= threshold {
                    format!("价格提醒：{} → {} 当前最低 ¥{:.0}，已低于阈值 ¥{:.0}！", from_city, to_city, price, threshold)
                } else {
                    format!("{} → {} 当前最低 ¥{:.0}，阈值 ¥{:.0}，继续监控中", from_city, to_city, price, threshold)
                }
            }));
        }
    }

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "checked": subs.len(),
        "alerts": alerts,
    }))
    .unwrap_or_default())
}

fn get_cheapest_train_price(
    db: &Database,
    scraper_base_url: &str,
    from_city: &str,
    to_city: &str,
    date: &str,
) -> Option<f64> {
    // Try cache first
    if let Ok(results) = db.search_trains(from_city, to_city, date, 120) {
        if let Some(best) = results.iter().filter_map(|r| r.lowest_price).reduce(f64::min) {
            return Some(best);
        }
    }

    // Try live
    let trains = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            crate::scrape::scrape_trains(scraper_base_url, from_city, to_city, date).await
        })
    })
    .ok()?;

    trains
        .iter()
        .flat_map(|t| t.seats.iter().map(|s| s.price))
        .reduce(f64::min)
}

fn get_cheapest_flight_price(
    db: &Database,
    scraper_base_url: &str,
    from_city: &str,
    to_city: &str,
    date: &str,
) -> Option<f64> {
    // Try cache first
    if let Ok(results) = db.search_flights(from_city, to_city, date, 120) {
        if let Some(best) = results.iter().filter_map(|r| r.lowest_price).reduce(f64::min) {
            return Some(best);
        }
    }

    // Try live
    let flights = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            crate::scrape::scrape_flights(scraper_base_url, from_city, to_city, date).await
        })
    })
    .ok()?;

    flights
        .iter()
        .flat_map(|f| f.prices.iter().map(|p| p.price))
        .reduce(f64::min)
}
