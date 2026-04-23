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
    _db: &Database,
    _scraper_base_url: &str,
    input: &str,
) -> Result<String, RuntimeError> {
    let params: CompareRoutesParams =
        serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
            tool_name: "compare_routes".into(),
            message: format!("Invalid input: {e}"),
        })?;

    info!(
        "Comparing routes: {} -> {} on {}, priority: {:?}",
        params.from_city, params.to_city, params.travel_date, params.priority
    );

    // TODO: 实际实现需要：
    // 1. 调用 search_trains 获取火车票
    // 2. 调用 search_flights 获取机票
    // 3. 计算总时间（包括市区到机场/车站的时间）
    // 4. 多维度评分（时间、费用、舒适度）
    // 5. 生成 2-3 个推荐方案

    // 简化实现：返回 mock 对比结果
    let priority = params.priority.as_deref().unwrap_or("cost");

    let comparison = serde_json::json!({
        "routes": [
            {
                "type": "高铁",
                "train_id": "G1234",
                "time": {
                    "depart": "08:00",
                    "arrive": "16:30",
                    "total_minutes": 510,
                    "description": "8小时30分"
                },
                "cost": {
                    "ticket": 650.5,
                    "transport": 0,
                    "total": 650.5,
                    "description": "¥650.5（二等座）"
                },
                "comfort": {
                    "score": 8,
                    "description": "舒适，市区直达，准点率高"
                },
                "recommended": priority == "cost" || priority == "comfort"
            },
            {
                "type": "飞机",
                "flight_id": "CA1234",
                "time": {
                    "depart": "09:00",
                    "arrive": "12:30",
                    "flight_minutes": 210,
                    "airport_time": 120,
                    "total_minutes": 330,
                    "description": "3小时30分飞行 + 2小时机场时间 = 5小时30分"
                },
                "cost": {
                    "ticket": 850.0,
                    "transport": 100,
                    "total": 950.0,
                    "description": "¥850（经济舱）+ ¥100（机场交通）"
                },
                "comfort": {
                    "score": 7,
                    "description": "较舒适，需提前到机场"
                },
                "recommended": priority == "time"
            }
        ],
        "recommendation": {
            "priority": priority,
            "choice": if priority == "time" { "飞机" } else { "高铁" },
            "reason": if priority == "time" {
                "飞机总时间最短（5.5小时），适合赶时间的行程"
            } else if priority == "cost" {
                "高铁性价比最高（¥650.5），且市区直达更方便"
            } else {
                "高铁舒适度更高，准点率高，市区直达"
            }
        }
    });

    Ok(serde_json::to_string_pretty(&comparison).unwrap_or_default())
}
