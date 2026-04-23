use runtime::ToolSpec;

/// Build the tool definitions for v0.2
#[must_use]
pub fn all_tool_specs() -> Vec<ToolSpec> {
    vec![
        // v0.1 tools
        scrape_hotels_spec(),
        search_hotels_spec(),
        analyze_prices_spec(),
        export_report_spec(),
        // v0.2 tools
        search_trains_spec(),
        search_flights_spec(),
        compare_routes_spec(),
        query_city_info_spec(),
    ]
}

fn scrape_hotels_spec() -> ToolSpec {
    ToolSpec {
        name: "scrape_hotels".to_string(),
        description: "从携程爬取指定城市和日期范围的酒店列表。\
                      调用 Python 爬虫服务处理反爬绕过和浏览器自动化。\
                      \n\n**使用建议**: 调用前应主动询问用户：\
                      1. 需要什么排序方式（价格、评分、推荐度）\
                      2. 需要爬取多少页数据（建议 5-10 页，最多 50 页）\
                      \n\n**限制**: 单次最多爬取 50 页，间隔 3 秒。\
                      同一城市同一日期范围，24 小时内只能爬取一次。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称（中文或拼音）或携程城市 ID",
                    "minLength": 1,
                    "maxLength": 50
                },
                "checkin": {
                    "type": "string",
                    "description": "入住日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "checkout": {
                    "type": "string",
                    "description": "退房日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "max_pages": {
                    "type": "integer",
                    "description": "最大爬取页数（默认 10，建议 5-10 页，最大 50 页）",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 10
                }
            },
            "required": ["city", "checkin", "checkout"]
        }),
    }
}

fn search_hotels_spec() -> ToolSpec {
    ToolSpec {
        name: "search_hotels".to_string(),
        description: "从本地 SQLite 数据库搜索已爬取的酒店数据。\
                      优先使用此工具查询，避免重复爬取。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称",
                    "minLength": 1,
                    "maxLength": 50
                },
                "min_price": {
                    "type": "number",
                    "description": "最低价格（人民币）",
                    "minimum": 0
                },
                "max_price": {
                    "type": "number",
                    "description": "最高价格（人民币）",
                    "minimum": 0
                },
                "min_star": {
                    "type": "integer",
                    "description": "最低星级 (1-5)",
                    "minimum": 1,
                    "maximum": 5
                },
                "min_rating": {
                    "type": "number",
                    "description": "最低评分 (0-5)",
                    "minimum": 0,
                    "maximum": 5
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["price", "rating", "star"],
                    "description": "排序方式"
                },
                "limit": {
                    "type": "integer",
                    "description": "返回数量限制（默认 20，最大 100）",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            }
        }),
    }
}

fn analyze_prices_spec() -> ToolSpec {
    ToolSpec {
        name: "analyze_prices".to_string(),
        description: "分析价格趋势，跨多个快照对比酒店价格。\
                      帮助用户找到最佳预订时机。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "hotel_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "要分析的酒店 ID 列表",
                    "minItems": 1,
                    "maxItems": 10
                },
                "date_range": {
                    "type": "object",
                    "properties": {
                        "start": {
                            "type": "string",
                            "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                        },
                        "end": {
                            "type": "string",
                            "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                        }
                    },
                    "description": "日期范围 (YYYY-MM-DD)"
                },
                "comparison_type": {
                    "type": "string",
                    "enum": ["trend", "cheapest", "best_value"],
                    "description": "分析类型：trend=趋势分析，cheapest=最低价，best_value=性价比",
                    "default": "trend"
                }
            },
            "required": ["hotel_ids"]
        }),
    }
}

fn export_report_spec() -> ToolSpec {
    ToolSpec {
        name: "export_report".to_string(),
        description: "将爬取数据导出为 CSV 或 JSON 文件。\
                      文件保存在 data/ 目录下。\
                      \n\n**限制**: 导出文件大小不超过 50MB。".to_string(),
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
                    "description": "按城市筛选（可选）",
                    "minLength": 1,
                    "maxLength": 50
                }
            },
            "required": ["format"]
        }),
    }
}

// ============================================================
// v0.2 Tool Specs
// ============================================================

fn search_trains_spec() -> ToolSpec {
    ToolSpec {
        name: "search_trains".to_string(),
        description: "查询指定路线和日期的火车票信息。\
                      支持按车型、时间、价格筛选。\
                      数据来源：12306 官网（当前使用 mock 数据）。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "from_city": {
                    "type": "string",
                    "description": "出发城市",
                    "minLength": 1,
                    "maxLength": 50
                },
                "to_city": {
                    "type": "string",
                    "description": "到达城市",
                    "minLength": 1,
                    "maxLength": 50
                },
                "travel_date": {
                    "type": "string",
                    "description": "出行日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "train_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["G", "D", "C", "K", "T", "Z"]
                    },
                    "description": "车型筛选（可选）"
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["time", "price", "duration"],
                    "description": "排序方式",
                    "default": "time"
                },
                "limit": {
                    "type": "integer",
                    "description": "返回数量限制",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 20
                }
            },
            "required": ["from_city", "to_city", "travel_date"]
        }),
    }
}

fn search_flights_spec() -> ToolSpec {
    ToolSpec {
        name: "search_flights".to_string(),
        description: "查询指定路线和日期的机票信息。\
                      支持按航司、舱位、价格筛选。\
                      数据来源：携程等多源聚合（当前使用 mock 数据）。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "from_city": {
                    "type": "string",
                    "description": "出发城市",
                    "minLength": 1,
                    "maxLength": 50
                },
                "to_city": {
                    "type": "string",
                    "description": "到达城市",
                    "minLength": 1,
                    "maxLength": 50
                },
                "travel_date": {
                    "type": "string",
                    "description": "出行日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "cabin_class": {
                    "type": "string",
                    "enum": ["economy", "business", "first"],
                    "description": "舱位等级（可选）"
                },
                "max_price": {
                    "type": "number",
                    "description": "最高价格（可选）",
                    "minimum": 0
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["time", "price", "duration"],
                    "description": "排序方式",
                    "default": "price"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 20
                }
            },
            "required": ["from_city", "to_city", "travel_date"]
        }),
    }
}

fn compare_routes_spec() -> ToolSpec {
    ToolSpec {
        name: "compare_routes".to_string(),
        description: "对比飞机、高铁、普通火车等交通方式。\
                      从时间、费用、舒适度多维度评分。\
                      自动生成 2-3 个推荐方案。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "from_city": {
                    "type": "string",
                    "description": "出发城市"
                },
                "to_city": {
                    "type": "string",
                    "description": "到达城市"
                },
                "travel_date": {
                    "type": "string",
                    "description": "出行日期 (YYYY-MM-DD)"
                },
                "budget": {
                    "type": "number",
                    "description": "预算（元，可选）"
                },
                "priority": {
                    "type": "string",
                    "enum": ["time", "cost", "comfort"],
                    "description": "优先级（时间/费用/舒适度）",
                    "default": "cost"
                }
            },
            "required": ["from_city", "to_city", "travel_date"]
        }),
    }
}

fn query_city_info_spec() -> ToolSpec {
    ToolSpec {
        name: "query_city_info".to_string(),
        description: "查询城市的地理信息、区域划分、主要景点。\
                      帮助用户了解城市布局，选择合适的住宿区域。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称"
                },
                "info_type": {
                    "type": "string",
                    "enum": ["overview", "districts", "attractions", "transport"],
                    "description": "信息类型",
                    "default": "overview"
                },
                "category": {
                    "type": "string",
                    "description": "景点类别筛选（仅当 info_type=attractions 时有效）"
                }
            },
            "required": ["city"]
        }),
    }
}
