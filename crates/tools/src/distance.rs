use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
pub struct CityDistanceParams {
    pub city: String,
    pub target_city: Option<String>,
    pub radius_km: Option<f64>,
    pub limit: Option<usize>,
}

/// Haversine formula to calculate distance between two points on Earth.
fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0; // Earth radius in km
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    R * c
}

pub fn handle_city_distance(
    db: &Database,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: CityDistanceParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "city_distance".into(),
            message: format!("Invalid input: {e}"),
        })?;

    info!("City distance query: {}", params.city);

    // Resolve the source city
    let source_city = db.resolve_city(&params.city).map_err(|e| RuntimeError::Tool {
        tool_name: "city_distance".into(),
        message: e.to_string(),
    })?;

    let source_city = source_city.ok_or_else(|| RuntimeError::Tool {
        tool_name: "city_distance".into(),
        message: format!("未找到城市: {}", params.city),
    })?;

    let (src_lat, src_lon) = match (source_city.latitude, source_city.longitude) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => {
            return Err(RuntimeError::Tool {
                tool_name: "city_distance".into(),
                message: format!("城市 {} 缺少经纬度数据", source_city.name),
            });
        }
    };

    // Mode 1: Distance between two specific cities
    if let Some(target_name) = &params.target_city {
        let target_city = db.resolve_city(target_name).map_err(|e| RuntimeError::Tool {
            tool_name: "city_distance".into(),
            message: e.to_string(),
        })?;

        let target_city = target_city.ok_or_else(|| RuntimeError::Tool {
            tool_name: "city_distance".into(),
            message: format!("未找到城市: {}", target_name),
        })?;

        let (tgt_lat, tgt_lon) = match (target_city.latitude, target_city.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => {
                return Err(RuntimeError::Tool {
                    tool_name: "city_distance".into(),
                    message: format!("城市 {} 缺少经纬度数据", target_city.name),
                });
            }
        };

        let distance = haversine_km(src_lat, src_lon, tgt_lat, tgt_lon);

        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "from": source_city.name,
            "to": target_city.name,
            "distance_km": (distance * 10.0).round() / 10.0,
            "from_coords": { "lat": src_lat, "lon": src_lon },
            "to_coords": { "lat": tgt_lat, "lon": tgt_lon },
        }))
        .unwrap_or_default());
    }

    // Mode 2: Find nearby cities within radius
    let radius_km = params.radius_km.unwrap_or(300.0).min(2000.0);
    let limit = params.limit.unwrap_or(10).min(50);

    let all_cities = db.list_cities_with_coords().map_err(|e| RuntimeError::Tool {
        tool_name: "city_distance".into(),
        message: e.to_string(),
    })?;

    let mut nearby: Vec<(String, Option<String>, f64)> = all_cities
        .iter()
        .filter(|c| c.id != source_city.id)
        .filter_map(|c| {
            let (lat, lon) = (c.latitude?, c.longitude?);
            let dist = haversine_km(src_lat, src_lon, lat, lon);
            if dist <= radius_km {
                Some((c.name.clone(), c.province.clone(), dist))
            } else {
                None
            }
        })
        .collect();

    nearby.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    nearby.truncate(limit);

    let cities: Vec<serde_json::Value> = nearby
        .iter()
        .map(|(name, province, dist)| {
            serde_json::json!({
                "city": name,
                "province": province,
                "distance_km": (*dist * 10.0).round() / 10.0,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "center": source_city.name,
        "radius_km": radius_km,
        "total": cities.len(),
        "nearby_cities": cities,
    }))
    .unwrap_or_default())
}
