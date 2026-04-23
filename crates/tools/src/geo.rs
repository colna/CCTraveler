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
    _db: &Database,
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

    // TODO: 实际实现需要从数据库查询城市地理数据
    // 当前返回 mock 数据

    let result = match info_type {
        "overview" => {
            serde_json::json!({
                "city": params.city,
                "province": "贵州省",
                "tier": 3,
                "population": 6600000,
                "description": "遵义是贵州省第二大城市，以遵义会议闻名，是红色旅游胜地。",
                "districts": ["红花岗区", "汇川区", "播州区"],
                "main_attractions": ["遵义会议旧址", "红军山", "海龙屯"]
            })
        }
        "districts" => {
            serde_json::json!({
                "city": params.city,
                "districts": [
                    {
                        "name": "汇川区",
                        "description": "市中心，交通便利，商业发达",
                        "tags": ["商业区", "交通枢纽", "会议旧址"]
                    },
                    {
                        "name": "红花岗区",
                        "description": "老城区，历史文化浓厚",
                        "tags": ["历史文化", "红色旅游"]
                    },
                    {
                        "name": "播州区",
                        "description": "新区，环境优美",
                        "tags": ["新区", "生态"]
                    }
                ],
                "recommendation": "建议住汇川区，距离主要景点近，交通便利"
            })
        }
        "attractions" => {
            serde_json::json!({
                "city": params.city,
                "attractions": [
                    {
                        "name": "遵义会议旧址",
                        "category": "历史",
                        "rating": 4.7,
                        "ticket_price": 0,
                        "visit_duration": 2.0,
                        "description": "中国革命历史的重要转折点"
                    },
                    {
                        "name": "红军山",
                        "category": "历史",
                        "rating": 4.5,
                        "ticket_price": 0,
                        "visit_duration": 1.5,
                        "description": "红军烈士陵园"
                    },
                    {
                        "name": "海龙屯",
                        "category": "历史",
                        "rating": 4.6,
                        "ticket_price": 80,
                        "visit_duration": 3.0,
                        "description": "世界文化遗产，土司遗址"
                    }
                ]
            })
        }
        "transport" => {
            serde_json::json!({
                "city": params.city,
                "airports": ["遵义新舟机场"],
                "train_stations": ["遵义站", "遵义西站"],
                "metro": null,
                "bus": "公交系统完善",
                "taxi": "起步价 8 元"
            })
        }
        _ => {
            return Err(RuntimeError::Tool {
                tool_name: "query_city_info".into(),
                message: format!("Unknown info_type: {}", info_type),
            });
        }
    };

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}
