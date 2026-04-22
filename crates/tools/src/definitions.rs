use runtime::ToolSpec;

/// Build the 4 tool definitions per architecture section 10.
#[must_use] 
pub fn all_tool_specs() -> Vec<ToolSpec> {
    vec![
        scrape_hotels_spec(),
        search_hotels_spec(),
        analyze_prices_spec(),
        export_report_spec(),
    ]
}

fn scrape_hotels_spec() -> ToolSpec {
    ToolSpec {
        name: "scrape_hotels".to_string(),
        description: "从携程爬取指定城市和日期范围的酒店列表。调用 Python 爬虫服务处理反爬绕过和浏览器自动化。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称（中文或拼音）或携程城市 ID"
                },
                "checkin": {
                    "type": "string",
                    "description": "入住日期 (YYYY-MM-DD)"
                },
                "checkout": {
                    "type": "string",
                    "description": "退房日期 (YYYY-MM-DD)"
                },
                "max_pages": {
                    "type": "integer",
                    "description": "最大爬取页数（默认 5）"
                }
            },
            "required": ["city", "checkin", "checkout"]
        }),
    }
}

fn search_hotels_spec() -> ToolSpec {
    ToolSpec {
        name: "search_hotels".to_string(),
        description: "从本地 SQLite 数据库搜索已爬取的酒店数据。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称"
                },
                "min_price": {
                    "type": "number",
                    "description": "最低价格"
                },
                "max_price": {
                    "type": "number",
                    "description": "最高价格"
                },
                "min_star": {
                    "type": "integer",
                    "description": "最低星级 (1-5)"
                },
                "min_rating": {
                    "type": "number",
                    "description": "最低评分"
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["price", "rating", "star"],
                    "description": "排序方式"
                },
                "limit": {
                    "type": "integer",
                    "description": "返回数量限制"
                }
            }
        }),
    }
}

fn analyze_prices_spec() -> ToolSpec {
    ToolSpec {
        name: "analyze_prices".to_string(),
        description: "分析价格趋势，跨多个快照对比酒店价格。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "hotel_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "要分析的酒店 ID 列表"
                },
                "date_range": {
                    "type": "object",
                    "properties": {
                        "start": { "type": "string" },
                        "end": { "type": "string" }
                    },
                    "description": "日期范围 (YYYY-MM-DD)"
                },
                "comparison_type": {
                    "type": "string",
                    "enum": ["trend", "cheapest", "best_value"],
                    "description": "分析类型"
                }
            },
            "required": ["hotel_ids"]
        }),
    }
}

fn export_report_spec() -> ToolSpec {
    ToolSpec {
        name: "export_report".to_string(),
        description: "将爬取数据导出为 CSV 或 JSON 文件。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["csv", "json"],
                    "description": "导出格式"
                },
                "city": {
                    "type": "string",
                    "description": "按城市筛选"
                }
            },
            "required": ["format"]
        }),
    }
}
