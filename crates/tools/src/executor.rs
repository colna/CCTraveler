use crate::definitions::all_tool_specs;
use crate::scrape::{ScrapeRequest, ScrapedHotel};
use runtime::types::{RuntimeError, ToolExecutor, ToolSpec};
use serde::Deserialize;
use storage::models::{Hotel, PriceSnapshot, Room, SearchFilters, SortBy};
use storage::Database;
use tracing::{info, warn};

/// `TravelerToolExecutor` — dispatches tool calls to scrape/search/analyze/export handlers.
///
/// Holds ownership of the Database and scraper URL.
pub struct TravelerToolExecutor {
    db: Database,
    scraper_base_url: String,
}

impl TravelerToolExecutor {
    pub fn new(db: Database, scraper_base_url: String) -> Self {
        Self {
            db,
            scraper_base_url,
        }
    }

    fn handle_scrape(&mut self, input: &str) -> Result<String, RuntimeError> {
        let params: ScrapeParams =
            serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: format!("Invalid input: {e}"),
            })?;

        // 1. 验证日期格式和合理性
        let checkin = chrono::NaiveDate::parse_from_str(&params.checkin, "%Y-%m-%d")
            .map_err(|_| RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "入住日期格式错误，应为 YYYY-MM-DD".into(),
            })?;
        let checkout = chrono::NaiveDate::parse_from_str(&params.checkout, "%Y-%m-%d")
            .map_err(|_| RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "退房日期格式错误，应为 YYYY-MM-DD".into(),
            })?;

        if checkout <= checkin {
            return Err(RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "退房日期必须晚于入住日期".into(),
            });
        }

        if (checkout - checkin).num_days() > 30 {
            return Err(RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "住宿天数不能超过 30 天".into(),
            });
        }

        // 2. 限制 max_pages 不超过 5
        let max_pages = params.max_pages.unwrap_or(5).min(5);

        // 3. 检查爬取频率（24 小时内不重复爬取同一城市同一日期）
        // TODO: 实现 get_last_scrape_time 检查
        // if let Some(last_scrape) = self.get_last_scrape_time(&params.city, &params.checkin)? {
        //     let elapsed = chrono::Utc::now().signed_duration_since(last_scrape);
        //     if elapsed.num_hours() < 24 {
        //         return Ok(format!(
        //             "该城市和日期的数据在 {} 小时前已爬取，请使用 search_hotels 查询本地数据。",
        //             elapsed.num_hours()
        //         ));
        //     }
        // }

        let req = ScrapeRequest {
            city: params.city.clone(),
            checkin: params.checkin.clone(),
            checkout: params.checkout.clone(),
            max_pages,
            source: "trip".to_string(),
        };

        // Bridge async scrape to sync via block_in_place
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                crate::scrape::scrape_hotels(&self.scraper_base_url, &req).await
            })
        })
        .map_err(|e| RuntimeError::Tool {
            tool_name: "scrape_hotels".into(),
            message: e.to_string(),
        })?;

        info!("Scraped {} hotels for {}", resp.hotels.len(), params.city);

        // Store in database
        let now = chrono::Utc::now().to_rfc3339();
        for hotel in &resp.hotels {
            if let Err(e) = store_scraped_hotel(
                &self.db,
                hotel,
                &params.city,
                &params.checkin,
                &params.checkout,
                &now,
            ) {
                warn!("Failed to store hotel {}: {e}", hotel.id);
            }
        }

        // Build summary (first 10 hotels for the LLM)
        let summary = build_scrape_summary(&resp.hotels, &params.city);
        Ok(summary)
    }

    fn handle_search(&self, input: &str) -> Result<String, RuntimeError> {
        let params: SearchParams =
            serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
                tool_name: "search_hotels".into(),
                message: format!("Invalid input: {e}"),
            })?;

        // 验证参数合理性
        if let Some(min_price) = params.min_price {
            if min_price < 0.0 {
                return Err(RuntimeError::Tool {
                    tool_name: "search_hotels".into(),
                    message: "最低价格不能为负数".into(),
                });
            }
        }

        if let Some(max_price) = params.max_price {
            if max_price < 0.0 {
                return Err(RuntimeError::Tool {
                    tool_name: "search_hotels".into(),
                    message: "最高价格不能为负数".into(),
                });
            }
            if let Some(min_price) = params.min_price {
                if max_price < min_price {
                    return Err(RuntimeError::Tool {
                        tool_name: "search_hotels".into(),
                        message: "最高价格不能低于最低价格".into(),
                    });
                }
            }
        }

        if let Some(min_star) = params.min_star {
            if !(1..=5).contains(&min_star) {
                return Err(RuntimeError::Tool {
                    tool_name: "search_hotels".into(),
                    message: "星级必须在 1-5 之间".into(),
                });
            }
        }

        if let Some(min_rating) = params.min_rating {
            if !(0.0..=5.0).contains(&min_rating) {
                return Err(RuntimeError::Tool {
                    tool_name: "search_hotels".into(),
                    message: "评分必须在 0-5 之间".into(),
                });
            }
        }

        // 限制返回数量不超过 100
        let limit = params.limit.unwrap_or(20).min(100);

        let filters = SearchFilters {
            city: params.city,
            min_price: params.min_price,
            max_price: params.max_price,
            min_star: params.min_star,
            min_rating: params.min_rating,
            sort_by: params.sort_by.as_deref().map(parse_sort_by),
            limit: Some(limit),
        };

        let results = self.db.search_hotels(&filters).map_err(|e| RuntimeError::Tool {
            tool_name: "search_hotels".into(),
            message: e.to_string(),
        })?;

        if results.is_empty() {
            return Ok("本地数据库中没有找到匹配的酒店。可能需要先爬取数据。".to_string());
        }

        // Build JSON summary
        let hotels: Vec<serde_json::Value> = results
            .iter()
            .map(|h| {
                serde_json::json!({
                    "name": h.hotel.name,
                    "star": h.hotel.star,
                    "rating": h.hotel.rating,
                    "price": h.lowest_price,
                    "room": h.room_name,
                    "address": h.hotel.address,
                    "city": h.hotel.city,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "total": results.len(),
            "hotels": hotels
        }))
        .unwrap_or_default())
    }

    fn handle_analyze(&self, input: &str) -> Result<String, RuntimeError> {
        let params: AnalyzeParams =
            serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
                tool_name: "analyze_prices".into(),
                message: format!("Invalid input: {e}"),
            })?;

        // 验证酒店 ID 数量不超过 10
        if params.hotel_ids.is_empty() {
            return Err(RuntimeError::Tool {
                tool_name: "analyze_prices".into(),
                message: "至少需要提供一个酒店 ID".into(),
            });
        }

        if params.hotel_ids.len() > 10 {
            return Err(RuntimeError::Tool {
                tool_name: "analyze_prices".into(),
                message: "最多只能同时分析 10 个酒店".into(),
            });
        }

        let mut analysis = Vec::new();

        for hotel_id in &params.hotel_ids {
            let filters = SearchFilters {
                city: None,
                min_price: None,
                max_price: None,
                min_star: None,
                min_rating: None,
                sort_by: None,
                limit: Some(1),
            };

            // Get hotel info
            let results = self.db.search_hotels(&filters).map_err(|e| RuntimeError::Tool {
                tool_name: "analyze_prices".into(),
                message: e.to_string(),
            })?;

            if let Some(hotel) = results.iter().find(|h| h.hotel.id == *hotel_id) {
                analysis.push(serde_json::json!({
                    "hotel_id": hotel_id,
                    "name": hotel.hotel.name,
                    "star": hotel.hotel.star,
                    "rating": hotel.hotel.rating,
                    "lowest_price": hotel.lowest_price,
                    "original_price": hotel.original_price,
                    "discount": hotel.original_price.and_then(|orig| {
                        hotel.lowest_price.map(|low| {
                            format!("{:.0}%", (1.0 - low / orig) * 100.0)
                        })
                    }),
                }));
            }
        }

        if analysis.is_empty() {
            return Ok("未找到指定酒店的价格数据。".to_string());
        }

        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "analysis": analysis,
            "comparison_type": params.comparison_type.unwrap_or_else(|| "trend".into()),
        }))
        .unwrap_or_default())
    }

    fn handle_export(&self, input: &str) -> Result<String, RuntimeError> {
        let params: ExportParams =
            serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
                tool_name: "export_report".into(),
                message: format!("Invalid input: {e}"),
            })?;

        let filters = SearchFilters {
            city: params.city,
            ..Default::default()
        };

        let content = match params.format.as_str() {
            "csv" => self.db.export_csv(&filters),
            "json" => self.db.export_json(&filters),
            other => {
                return Err(RuntimeError::Tool {
                    tool_name: "export_report".into(),
                    message: format!("Unsupported format: {other}. Use csv or json."),
                })
            }
        }
        .map_err(|e| RuntimeError::Tool {
            tool_name: "export_report".into(),
            message: e.to_string(),
        })?;

        // 检查文件大小不超过 50MB
        const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50MB
        if content.len() > MAX_FILE_SIZE {
            return Err(RuntimeError::Tool {
                tool_name: "export_report".into(),
                message: format!(
                    "导出文件过大（{:.2} MB），超过 50MB 限制。请使用筛选条件减少数据量。",
                    content.len() as f64 / 1024.0 / 1024.0
                ),
            });
        }

        // Save to file
        let filename = format!(
            "hotels_export_{}.{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            params.format
        );
        let path = std::path::Path::new("data").join(&filename);
        std::fs::create_dir_all("data").ok();
        std::fs::write(&path, &content).map_err(|e| RuntimeError::Tool {
            tool_name: "export_report".into(),
            message: format!("Failed to write file: {e}"),
        })?;

        Ok(format!(
            "已导出到 {path}（{:.2} MB）",
            content.len() as f64 / 1024.0 / 1024.0,
            path = path.display()
        ))
    }
}

impl ToolExecutor for TravelerToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, RuntimeError> {
        match tool_name {
            "scrape_hotels" => self.handle_scrape(input),
            "search_hotels" => self.handle_search(input),
            "analyze_prices" => self.handle_analyze(input),
            "export_report" => self.handle_export(input),
            other => Err(RuntimeError::Tool {
                tool_name: other.to_string(),
                message: "Unknown tool".to_string(),
            }),
        }
    }

    fn tool_specs(&self) -> Vec<ToolSpec> {
        all_tool_specs()
    }
}

// ─── Parameter types ───

#[derive(Deserialize)]
struct ScrapeParams {
    city: String,
    checkin: String,
    checkout: String,
    max_pages: Option<u32>,
}

#[derive(Deserialize)]
struct SearchParams {
    city: Option<String>,
    min_price: Option<f64>,
    max_price: Option<f64>,
    min_star: Option<u8>,
    min_rating: Option<f64>,
    sort_by: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct AnalyzeParams {
    hotel_ids: Vec<String>,
    comparison_type: Option<String>,
}

#[derive(Deserialize)]
struct ExportParams {
    format: String,
    city: Option<String>,
}

// ─── Helpers ───

fn parse_sort_by(s: &str) -> SortBy {
    match s {
        "rating" => SortBy::Rating,
        "star" => SortBy::Star,
        _ => SortBy::Price,
    }
}

/// Store a scraped hotel + its rooms + prices into the database.
pub fn store_scraped_hotel(
    db: &Database,
    hotel: &ScrapedHotel,
    city: &str,
    checkin: &str,
    checkout: &str,
    now: &str,
) -> anyhow::Result<()> {
    let h = Hotel {
        id: hotel.id.clone(),
        name: hotel.name.clone(),
        name_en: hotel.name_en.clone(),
        star: hotel.star,
        rating: hotel.rating,
        rating_count: hotel.rating_count.unwrap_or(0),
        address: hotel.address.clone(),
        latitude: hotel.latitude,
        longitude: hotel.longitude,
        image_url: hotel.image_url.clone(),
        amenities: vec![],
        city: city.to_string(),
        district: hotel.district.clone(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    };
    db.upsert_hotel(&h)?;

    for (i, room) in hotel.rooms.iter().enumerate() {
        let room_id = format!("{}-room-{i}", hotel.id);
        let r = Room {
            id: room_id.clone(),
            hotel_id: hotel.id.clone(),
            name: room.name.clone(),
            bed_type: room.bed_type.clone(),
            max_guests: 2,
            area: None,
            has_window: false,
            has_breakfast: room.has_breakfast.unwrap_or(false),
            cancellation_policy: room
                .has_free_cancel
                .and_then(|v| if v { Some("免费取消".to_string()) } else { None }),
        };
        db.insert_room(&r)?;

        if let Some(price_val) = room.price {
            let price = PriceSnapshot {
                id: uuid::Uuid::new_v4().to_string(),
                room_id,
                hotel_id: hotel.id.clone(),
                price: price_val,
                original_price: room.original_price,
                checkin: checkin.to_string(),
                checkout: checkout.to_string(),
                scraped_at: now.to_string(),
                source: "trip.com".to_string(),
            };
            db.insert_price(&price)?;
        }
    }

    Ok(())
}

/// Build a summary string of scraped hotels for the LLM (max 10).
fn build_scrape_summary(hotels: &[ScrapedHotel], city: &str) -> String {
    let mut lines = vec![format!("成功爬取 {city} 的 {} 家酒店：", hotels.len())];

    for (i, h) in hotels.iter().take(10).enumerate() {
        let star = h.star.map_or("-".into(), |s| "★".repeat(s as usize));
        let rating = h.rating.map_or("-".into(), |r| format!("{r:.1}"));
        let min_price = h
            .rooms
            .iter()
            .filter_map(|r| r.price)
            .reduce(f64::min);
        let price_str = min_price.map_or("价格未知".into(), |p| format!("¥{p:.0}"));
        let room_count = h.rooms.len();
        lines.push(format!(
            "{}. {} {star} 评分:{rating} {price_str} ({room_count}个房型)",
            i + 1,
            h.name
        ));
    }

    if hotels.len() > 10 {
        lines.push(format!("... 还有 {} 家酒店", hotels.len() - 10));
    }

    lines.push("\n数据已存入本地数据库，可用 search_hotels 进一步筛选。".to_string());
    lines.join("\n")
}
