use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
pub struct QueryCityInfoParams {
    pub city: String,
    pub info_type: Option<String>,
    pub category: Option<String>,
}

pub fn handle_query_city_info(
    db: &Database,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: QueryCityInfoParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "query_city_info".into(),
            message: format!("Invalid input: {e}"),
        })?;

    let info_type = params.info_type.as_deref().unwrap_or("overview");

    info!(
        "Querying city info: {} (type: {})",
        params.city, info_type
    );

    // 查询数据库
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            query_city_info_from_db(db, &params.city, info_type, params.category.as_deref()).await
        })
    })
    .map_err(|e| RuntimeError::Tool {
        tool_name: "query_city_info".into(),
        message: e.to_string(),
    })?;

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

async fn query_city_info_from_db(
    db: &Database,
    city: &str,
    info_type: &str,
    category: Option<&str>,
) -> Result<serde_json::Value, anyhow::Error> {
    let conn = &db.conn;

    match info_type {
        "overview" => {
            // 查询城市基本信息
            let city_info: Option<(String, String, i32, i64, f64, String)> = conn
                .query_row(
                    "SELECT name, province, tier, population, area_km2, description
                     FROM cities WHERE name = ?",
                    [city],
                    |row: &rusqlite::Row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .ok();

            if let Some((name, province, tier, population, area_km2, description)) = city_info {
                // 查询区域列表
                let mut stmt = conn.prepare(
                    "SELECT name FROM districts WHERE city_id = (SELECT id FROM cities WHERE name = ?)"
                )?;
                let districts: Vec<String> = stmt
                    .query_map([city], |row: &rusqlite::Row| row.get(0))?
                    .filter_map(Result::ok)
                    .collect();

                // 查询主要景点
                let mut stmt = conn.prepare(
                    "SELECT name FROM attractions WHERE city_id = (SELECT id FROM cities WHERE name = ?) LIMIT 5"
                )?;
                let attractions: Vec<String> = stmt
                    .query_map([city], |row: &rusqlite::Row| row.get(0))?
                    .filter_map(Result::ok)
                    .collect();

                Ok(serde_json::json!({
                    "city": name,
                    "province": province,
                    "tier": tier,
                    "population": population,
                    "area_km2": area_km2,
                    "description": description,
                    "districts": districts,
                    "main_attractions": attractions
                }))
            } else {
                Ok(serde_json::json!({
                    "error": format!("未找到城市: {}", city)
                }))
            }
        }
        "districts" => {
            // 查询区域详情
            let mut stmt = conn.prepare(
                "SELECT d.name, d.description, d.tags
                 FROM districts d
                 JOIN cities c ON d.city_id = c.id
                 WHERE c.name = ?"
            )?;

            let districts: Vec<serde_json::Value> = stmt
                .query_map([city], |row: &rusqlite::Row| {
                    let name: String = row.get(0)?;
                    let description: String = row.get(1)?;
                    let tags_str: String = row.get(2)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

                    Ok(serde_json::json!({
                        "name": name,
                        "description": description,
                        "tags": tags
                    }))
                })?
                .filter_map(Result::ok)
                .collect();

            if districts.is_empty() {
                Ok(serde_json::json!({
                    "city": city,
                    "districts": [],
                    "message": "暂无区域数据"
                }))
            } else {
                Ok(serde_json::json!({
                    "city": city,
                    "districts": districts,
                    "recommendation": format!("建议住{}，交通便利", districts[0]["name"].as_str().unwrap_or("市中心"))
                }))
            }
        }
        "attractions" => {
            // 查询景点
            let mut query = String::from(
                "SELECT a.name, a.category, a.rating, a.ticket_price, a.visit_duration_hours, a.description
                 FROM attractions a
                 JOIN cities c ON a.city_id = c.id
                 WHERE c.name = ?"
            );

            let params: Vec<&str> = if let Some(cat) = category {
                query.push_str(" AND a.category = ?");
                vec![city, cat]
            } else {
                vec![city]
            };

            let mut stmt = conn.prepare(&query)?;

            let attractions: Vec<serde_json::Value> = stmt
                .query_map(rusqlite::params_from_iter(params), |row: &rusqlite::Row| {
                    Ok(serde_json::json!({
                        "name": row.get::<_, String>(0)?,
                        "category": row.get::<_, String>(1)?,
                        "rating": row.get::<_, f64>(2)?,
                        "ticket_price": row.get::<_, f64>(3)?,
                        "visit_duration": row.get::<_, f64>(4)?,
                        "description": row.get::<_, String>(5)?
                    }))
                })?
                .filter_map(Result::ok)
                .collect();

            Ok(serde_json::json!({
                "city": city,
                "attractions": attractions
            }))
        }
        "transport" => {
            // 查询交通信息
            let mut stmt = conn.prepare(
                "SELECT station_name, station_code FROM station_codes WHERE city = ?"
            )?;
            let train_stations: Vec<String> = stmt
                .query_map([city], |row: &rusqlite::Row| row.get(0))?
                .filter_map(Result::ok)
                .collect();

            let mut stmt = conn.prepare(
                "SELECT airport_name, airport_code FROM airport_codes WHERE city = ?"
            )?;
            let airports: Vec<String> = stmt
                .query_map([city], |row: &rusqlite::Row| row.get(0))?
                .filter_map(Result::ok)
                .collect();

            Ok(serde_json::json!({
                "city": city,
                "train_stations": train_stations,
                "airports": airports,
                "metro": if city == "北京" || city == "上海" || city == "深圳" { "有地铁" } else { "无地铁" },
                "bus": "公交系统完善"
            }))
        }
        _ => {
            Err(anyhow::anyhow!("Unknown info_type: {}", info_type))
        }
    }
}
