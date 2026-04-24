use storage::Database;
use tools::executor::TravelerToolExecutor;
use runtime::types::ToolExecutor;

fn setup_db() -> Database {
    let db = Database::open_in_memory().unwrap();

    // Insert test cities
    db.conn.execute(
        "INSERT INTO cities (id, name, name_en, province, latitude, longitude, tier)
         VALUES ('bj', '北京', 'Beijing', '北京', 39.9042, 116.4074, 1)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO cities (id, name, name_en, province, latitude, longitude, tier)
         VALUES ('sh', '上海', 'Shanghai', '上海', 31.2304, 121.4737, 1)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO cities (id, name, name_en, province, latitude, longitude, tier)
         VALUES ('tj', '天津', 'Tianjin', '天津', 39.3434, 117.3616, 2)",
        [],
    ).unwrap();

    // Insert test city mappings
    db.conn.execute(
        "INSERT INTO city_mappings (city_id, source, source_id, source_name)
         VALUES ('bj', 'ctrip', '1', '北京')",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO city_mappings (city_id, source, source_id, source_name)
         VALUES ('sh', 'ctrip', '2', '上海')",
        [],
    ).unwrap();

    // Insert test attractions
    db.conn.execute(
        "INSERT INTO attractions (id, city_id, name, category, rating, ticket_price, visit_duration_hours)
         VALUES ('a1', 'bj', '故宫', '历史', 4.9, 60.0, 3.0)",
        [],
    ).unwrap();

    db
}

fn setup_executor() -> TravelerToolExecutor {
    let db = setup_db();
    TravelerToolExecutor::new(db, "http://localhost:8300".to_string())
}

#[test]
fn test_wiki_remember_recall() {
    let mut executor = setup_executor();

    // Remember
    let result = executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"user_history","key":"budget","value":"500-1000"}"#,
    ).unwrap();
    assert!(result.contains("500-1000"));

    // Recall
    let result = executor.execute(
        "wiki",
        r#"{"action":"recall","topic":"user_history","key":"budget"}"#,
    ).unwrap();
    assert!(result.contains("500-1000"));
}

#[test]
fn test_wiki_list_and_forget() {
    let mut executor = setup_executor();

    // Remember two items
    executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"prefs","key":"star","value":"4"}"#,
    ).unwrap();
    executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"prefs","key":"budget","value":"800"}"#,
    ).unwrap();

    // List
    let result = executor.execute(
        "wiki",
        r#"{"action":"list","topic":"prefs"}"#,
    ).unwrap();
    assert!(result.contains("\"total\": 2") || result.contains("\"total\":2"));

    // Forget
    executor.execute(
        "wiki",
        r#"{"action":"forget","topic":"prefs","key":"star"}"#,
    ).unwrap();

    // List again
    let result = executor.execute(
        "wiki",
        r#"{"action":"list","topic":"prefs"}"#,
    ).unwrap();
    assert!(result.contains("\"total\": 1") || result.contains("\"total\":1"));
}

#[test]
fn test_city_distance_two_cities() {
    let mut executor = setup_executor();

    let result = executor.execute(
        "city_distance",
        r#"{"city":"北京","target_city":"上海"}"#,
    ).unwrap();
    // Beijing to Shanghai is roughly 1060-1070 km
    assert!(result.contains("distance_km"));
    // Check it parsed to JSON
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    let dist = json["distance_km"].as_f64().unwrap();
    assert!(dist > 1000.0 && dist < 1200.0, "Distance should be ~1060km, got {dist}");
}

#[test]
fn test_city_distance_nearby() {
    let mut executor = setup_executor();

    let result = executor.execute(
        "city_distance",
        r#"{"city":"北京","radius_km":200,"limit":5}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    // Tianjin is ~120km from Beijing
    let cities = json["nearby_cities"].as_array().unwrap();
    assert!(!cities.is_empty(), "Should find Tianjin within 200km of Beijing");
    assert_eq!(cities[0]["city"], "天津");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_query_city_info_overview() {
    let mut executor = setup_executor();

    let result = executor.execute(
        "query_city_info",
        r#"{"city":"北京","info_type":"overview"}"#,
    ).unwrap();
    assert!(result.contains("北京") || result.contains("Beijing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_query_city_info_attractions() {
    let mut executor = setup_executor();

    let result = executor.execute(
        "query_city_info",
        r#"{"city":"北京","info_type":"attractions"}"#,
    ).unwrap();
    assert!(result.contains("故宫"));
}

#[test]
fn test_price_monitor_subscribe_list() {
    let mut executor = setup_executor();

    // Subscribe
    let result = executor.execute(
        "price_monitor",
        r#"{"action":"subscribe","from_city":"北京","to_city":"上海","transport_type":"train","threshold":500}"#,
    ).unwrap();
    assert!(result.contains("订阅成功") || result.contains("subscribe"));

    // List
    let result = executor.execute(
        "price_monitor",
        r#"{"action":"list"}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    let subs = json["subscriptions"].as_array().unwrap();
    assert_eq!(subs.len(), 1);
}

#[test]
fn test_search_hotels_empty_db() {
    let mut executor = setup_executor();

    let result = executor.execute(
        "search_hotels",
        r#"{"city":"北京"}"#,
    ).unwrap();
    assert!(result.contains("没有找到"));
}

#[test]
fn test_tool_specs_count() {
    let specs = tools::all_tool_specs();
    // v0.1: 4 + v0.2: 4 + v0.3: 4 (distance, monitor, planner, wiki) = 12
    assert_eq!(specs.len(), 12, "Should have 12 tool specs");

    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"wiki"));
    assert!(names.contains(&"city_distance"));
    assert!(names.contains(&"price_monitor"));
    assert!(names.contains(&"plan_trip"));
}

#[test]
fn test_unknown_tool() {
    let mut executor = setup_executor();
    let result = executor.execute("nonexistent_tool", "{}");
    assert!(result.is_err());
}

#[test]
fn test_invalid_input() {
    let mut executor = setup_executor();

    // Invalid JSON
    let result = executor.execute("search_trains", "not json");
    assert!(result.is_err());

    // Invalid date
    let result = executor.execute(
        "search_trains",
        r#"{"from_city":"北京","to_city":"上海","travel_date":"invalid-date"}"#,
    );
    assert!(result.is_err());
}

#[test]
fn test_redis_cache_disabled() {
    let cache = tools::cache::RedisCache::new(false, "", 3600);
    assert!(!cache.is_available());

    // Get should return None
    let result = cache.get_transport("train", "北京", "上海", "2026-05-01");
    assert!(result.is_none());

    // Set should be a no-op (not panic)
    cache.set_transport("train", "北京", "上海", "2026-05-01", "test");
}
