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
    let Some(resolved_city) = db.resolve_city(city)? else {
        return Ok(serde_json::json!({
            "error": format!("未找到城市: {}", city)
        }));
    };

    match info_type {
        "overview" => {
            let districts = db
                .list_city_districts(&resolved_city.id)?
                .into_iter()
                .map(|district| district.name)
                .collect::<Vec<_>>();

            let attractions = db
                .list_city_attractions(&resolved_city.id, None)?
                .into_iter()
                .take(5)
                .map(|attraction| attraction.name)
                .collect::<Vec<_>>();

            Ok(serde_json::json!({
                "city": resolved_city.name,
                "province": resolved_city.province,
                "tier": resolved_city.tier,
                "population": resolved_city.population,
                "area_km2": resolved_city.area_km2,
                "description": resolved_city.description,
                "districts": districts,
                "main_attractions": attractions
            }))
        }
        "districts" => {
            let districts = db
                .list_city_districts(&resolved_city.id)?
                .into_iter()
                .map(|district| {
                    let tags = district
                        .tags
                        .as_deref()
                        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                        .unwrap_or_default();

                    serde_json::json!({
                        "name": district.name,
                        "description": district.description,
                        "tags": tags
                    })
                })
                .collect::<Vec<_>>();

            if districts.is_empty() {
                Ok(serde_json::json!({
                    "city": resolved_city.name,
                    "districts": [],
                    "message": "暂无区域数据"
                }))
            } else {
                Ok(serde_json::json!({
                    "city": resolved_city.name,
                    "districts": districts,
                    "recommendation": format!("建议住{}，交通便利", districts[0]["name"].as_str().unwrap_or("市中心"))
                }))
            }
        }
        "attractions" => {
            let attractions = db
                .list_city_attractions(&resolved_city.id, category)?
                .into_iter()
                .map(|attraction| {
                    serde_json::json!({
                        "name": attraction.name,
                        "category": attraction.category,
                        "rating": attraction.rating,
                        "ticket_price": attraction.ticket_price,
                        "visit_duration": attraction.visit_duration_hours,
                        "description": attraction.description
                    })
                })
                .collect::<Vec<_>>();

            Ok(serde_json::json!({
                "city": resolved_city.name,
                "attractions": attractions
            }))
        }
        "transport" => {
            let train_stations = db
                .list_city_station_codes(&resolved_city.name)?
                .into_iter()
                .map(|station| station.station_name)
                .collect::<Vec<_>>();

            let airports = db
                .list_city_airport_codes(&resolved_city.name)?
                .into_iter()
                .map(|airport| airport.airport_name)
                .collect::<Vec<_>>();

            Ok(serde_json::json!({
                "city": resolved_city.name,
                "train_stations": train_stations,
                "airports": airports,
                "metro": if resolved_city.name == "北京" || resolved_city.name == "上海" || resolved_city.name == "深圳" { "有地铁" } else { "无地铁" },
                "bus": "公交系统完善"
            }))
        }
        _ => Err(anyhow::anyhow!("Unknown info_type: {}", info_type)),
    }
}
