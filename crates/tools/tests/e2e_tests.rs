//! End-to-end tests for CCTraveler.
//!
//! These tests exercise full pipelines through the executor,
//! verifying that tools correctly interact with storage, cache,
//! and return properly structured responses.

use runtime::types::ToolExecutor;
use storage::Database;
use tools::executor::TravelerToolExecutor;

// ── Test setup helpers ──

fn setup_db() -> Database {
    let db = Database::open_in_memory().unwrap();

    // Test cities with coordinates for distance calculations
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
    db.conn.execute(
        "INSERT INTO cities (id, name, name_en, province, latitude, longitude, tier)
         VALUES ('gz', '广州', 'Guangzhou', '广东', 23.1291, 113.2644, 1)",
        [],
    ).unwrap();

    // City mappings
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

    // Attractions
    db.conn.execute(
        "INSERT INTO attractions (id, city_id, name, category, rating, ticket_price, visit_duration_hours)
         VALUES ('a1', 'bj', '故宫', '历史', 4.9, 60.0, 3.0)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO attractions (id, city_id, name, category, rating, ticket_price, visit_duration_hours)
         VALUES ('a2', 'sh', '外滩', '景观', 4.7, 0.0, 2.0)",
        [],
    ).unwrap();

    // Hotels for plan_trip and search_hotels
    for (id, name, star, rating, city) in &[
        ("h1", "北京大饭店", 4, 4.5, "北京"),
        ("h2", "上海国际酒店", 5, 4.8, "上海"),
    ] {
        let h = storage::Hotel {
            id: id.to_string(),
            name: name.to_string(),
            name_en: None,
            star: Some(*star),
            rating: Some(*rating),
            rating_count: 200,
            address: None,
            latitude: None,
            longitude: None,
            image_url: None,
            amenities: vec![],
            city: city.to_string(),
            district: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        };
        db.upsert_hotel(&h).unwrap();
    }

    db
}

fn setup_executor() -> TravelerToolExecutor {
    let db = setup_db();
    TravelerToolExecutor::new(db, "http://localhost:8300".to_string())
}

// ── E2E: Flight cache pipeline ──

#[test]
fn e2e_flight_cache_pipeline() {
    let db = setup_db();

    // 1. Insert flight + price into DB (simulating a previous scrape)
    let now = chrono::Utc::now().to_rfc3339();
    let flight = storage::Flight {
        id: "CA1234".to_string(),
        airline: "中国国航".to_string(),
        from_airport: "PEK".to_string(),
        to_airport: "SHA".to_string(),
        from_city: "北京".to_string(),
        to_city: "上海".to_string(),
        depart_time: "09:00".to_string(),
        arrive_time: "12:30".to_string(),
        duration_minutes: 210,
        aircraft_type: Some("A320".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    db.upsert_flight(&flight).unwrap();

    let price = storage::FlightPrice {
        id: "fp-e2e-1".to_string(),
        flight_id: "CA1234".to_string(),
        cabin_class: "经济舱".to_string(),
        price: 850.0,
        discount: Some(0.8),
        available_seats: Some(50),
        travel_date: "2026-06-15".to_string(),
        scraped_at: now.clone(),
        source: "qunar".to_string(),
    };
    db.insert_flight_price(&price).unwrap();

    // 2. Create executor with pre-populated DB
    let mut executor = TravelerToolExecutor::new(db, "http://localhost:8300".to_string());

    // 3. Search flights - should hit DB cache (no scraper needed)
    let result = executor.execute(
        "search_flights",
        r#"{"from_city":"北京","to_city":"上海","travel_date":"2026-06-15"}"#,
    ).unwrap();

    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["source"], "cache", "Should serve from DB cache");
    assert!(json["total"].as_u64().unwrap() >= 1);

    let flights = json["flights"].as_array().unwrap();
    assert_eq!(flights[0]["flight_id"], "CA1234");
    assert_eq!(flights[0]["airline"], "中国国航");
}

// ── E2E: Flight with max price filter ──

#[test]
fn e2e_flight_max_price_filter() {
    let db = setup_db();
    let now = chrono::Utc::now().to_rfc3339();

    // Insert two flights with different prices
    for (id, airline, price) in &[("CA111", "中国国航", 600.0), ("MU222", "东方航空", 1200.0)] {
        db.upsert_flight(&storage::Flight {
            id: id.to_string(),
            airline: airline.to_string(),
            from_airport: "PEK".to_string(),
            to_airport: "SHA".to_string(),
            from_city: "北京".to_string(),
            to_city: "上海".to_string(),
            depart_time: "14:30".to_string(),
            arrive_time: "18:00".to_string(),
            duration_minutes: 210,
            aircraft_type: Some("B737".to_string()),
            created_at: now.clone(),
            updated_at: now.clone(),
        }).unwrap();

        db.insert_flight_price(&storage::FlightPrice {
            id: format!("fp-{id}"),
            flight_id: id.to_string(),
            cabin_class: "经济舱".to_string(),
            price: *price,
            discount: Some(0.8),
            available_seats: Some(50),
            travel_date: "2026-06-15".to_string(),
            scraped_at: now.clone(),
            source: "fliggy".to_string(),
        }).unwrap();
    }

    let mut executor = TravelerToolExecutor::new(db, "http://localhost:8300".to_string());

    // Filter with max price 800 — should only return the cheap flight
    let result = executor.execute(
        "search_flights",
        r#"{"from_city":"北京","to_city":"上海","travel_date":"2026-06-15","max_price":800}"#,
    ).unwrap();

    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    let flights = json["flights"].as_array().unwrap();
    assert_eq!(flights.len(), 1, "Should only find one flight under ¥800");
    assert_eq!(flights[0]["flight_id"], "CA111");
}

// ── E2E: Train cache pipeline ──

#[test]
fn e2e_train_cache_pipeline() {
    let db = setup_db();
    let now = chrono::Utc::now().to_rfc3339();

    let train = storage::Train {
        id: "G1234".to_string(),
        train_type: "G".to_string(),
        from_station: "北京南".to_string(),
        to_station: "上海虹桥".to_string(),
        from_city: "北京".to_string(),
        to_city: "上海".to_string(),
        depart_time: "08:00".to_string(),
        arrive_time: "12:30".to_string(),
        duration_minutes: 270,
        distance_km: Some(1318),
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    db.upsert_train(&train).unwrap();
    db.insert_train_price(&storage::TrainPrice {
        id: "tp-e2e".to_string(),
        train_id: "G1234".to_string(),
        seat_type: "二等座".to_string(),
        price: 553.0,
        available_seats: Some(100),
        travel_date: "2026-06-15".to_string(),
        scraped_at: now,
        source: "12306".to_string(),
    }).unwrap();

    let mut executor = TravelerToolExecutor::new(db, "http://localhost:8300".to_string());

    let result = executor.execute(
        "search_trains",
        r#"{"from_city":"北京","to_city":"上海","travel_date":"2026-06-15"}"#,
    ).unwrap();

    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["source"], "cache");
    let trains = json["trains"].as_array().unwrap();
    assert_eq!(trains[0]["train_id"], "G1234");
    assert_eq!(trains[0]["lowest_price"], 553.0);
}

// ── E2E: Price monitor full lifecycle ──

#[test]
fn e2e_price_monitor_full_lifecycle() {
    let mut executor = setup_executor();

    // 1. Subscribe to a route
    let sub_result = executor.execute(
        "price_monitor",
        r#"{"action":"subscribe","from_city":"北京","to_city":"上海","transport_type":"train","threshold":600}"#,
    ).unwrap();
    assert!(sub_result.contains("订阅成功") || sub_result.contains("subscribe"));

    // 2. Subscribe to a second route
    executor.execute(
        "price_monitor",
        r#"{"action":"subscribe","from_city":"上海","to_city":"广州","transport_type":"flight","threshold":1000}"#,
    ).unwrap();

    // 3. List - should have 2
    let list_result = executor.execute(
        "price_monitor",
        r#"{"action":"list"}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&list_result).unwrap();
    assert_eq!(json["subscriptions"].as_array().unwrap().len(), 2);

    // 4. Unsubscribe first
    let sub_id = json["subscriptions"][0]["id"].as_str().unwrap().to_string();
    executor.execute(
        "price_monitor",
        &format!(r#"{{"action":"unsubscribe","subscription_id":"{sub_id}"}}"#),
    ).unwrap();

    // 5. List - should have 1
    let list_result = executor.execute(
        "price_monitor",
        r#"{"action":"list"}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&list_result).unwrap();
    assert_eq!(json["subscriptions"].as_array().unwrap().len(), 1);
}

// ── E2E: Wiki → Plan Trip cross-tool workflow ──

#[tokio::test(flavor = "multi_thread")]
async fn e2e_wiki_then_plan_trip() {
    let mut executor = setup_executor();

    // 1. Store user preferences in wiki
    executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"travel_prefs","key":"budget","value":"3000"}"#,
    ).unwrap();
    executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"travel_prefs","key":"style","value":"comfort"}"#,
    ).unwrap();

    // 2. Verify preferences stored
    let prefs = executor.execute(
        "wiki",
        r#"{"action":"list","topic":"travel_prefs"}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&prefs).unwrap();
    assert_eq!(json["total"].as_u64().unwrap(), 2);

    // 3. Plan trip using city that has hotel data
    let plan = executor.execute(
        "plan_trip",
        r#"{"from_city":"北京","to_city":"上海","start_date":"2026-06-15","end_date":"2026-06-17","budget":3000}"#,
    ).unwrap();
    // Plan should contain budget breakdown or destination info
    assert!(
        plan.contains("budget") || plan.contains("预算") || plan.contains("¥") || plan.contains("上海"),
        "Plan should contain budget or destination information"
    );
}

// ── E2E: Distance → City Info cross-tool workflow ──

#[tokio::test(flavor = "multi_thread")]
async fn e2e_distance_then_city_info() {
    let mut executor = setup_executor();

    // 1. Find nearby cities from Beijing
    let nearby = executor.execute(
        "city_distance",
        r#"{"city":"北京","radius_km":200,"limit":5}"#,
    ).unwrap();
    let json: serde_json::Value = serde_json::from_str(&nearby).unwrap();
    let cities = json["nearby_cities"].as_array().unwrap();
    assert!(!cities.is_empty(), "Should find nearby cities");

    // 2. Get info about the closest city found
    let closest = cities[0]["city"].as_str().unwrap();
    let info = executor.execute(
        "query_city_info",
        &format!(r#"{{"city":"{closest}","info_type":"overview"}}"#),
    ).unwrap();
    assert!(
        info.contains(closest),
        "City info should contain city name: {closest}"
    );
}

// ── E2E: Redis cache disabled graceful degradation ──

#[test]
fn e2e_redis_disabled_still_works() {
    let db = setup_db();
    let now = chrono::Utc::now().to_rfc3339();

    // Insert a flight into DB
    db.upsert_flight(&storage::Flight {
        id: "CZ9999".to_string(),
        airline: "南方航空".to_string(),
        from_airport: "PEK".to_string(),
        to_airport: "CAN".to_string(),
        from_city: "北京".to_string(),
        to_city: "广州".to_string(),
        depart_time: "07:00".to_string(),
        arrive_time: "10:30".to_string(),
        duration_minutes: 210,
        aircraft_type: Some("A321".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
    }).unwrap();
    db.insert_flight_price(&storage::FlightPrice {
        id: "fp-redis-test".to_string(),
        flight_id: "CZ9999".to_string(),
        cabin_class: "经济舱".to_string(),
        price: 900.0,
        discount: None,
        available_seats: Some(30),
        travel_date: "2026-07-01".to_string(),
        scraped_at: now,
        source: "ctrip".to_string(),
    }).unwrap();

    // Create executor WITH disabled Redis
    let redis = tools::cache::RedisCache::new(false, "", 3600);
    let mut executor = TravelerToolExecutor::new(db, "http://localhost:8300".to_string())
        .with_redis(redis);

    // Should still serve from SQLite cache
    let result = executor.execute(
        "search_flights",
        r#"{"from_city":"北京","to_city":"广州","travel_date":"2026-07-01"}"#,
    ).unwrap();

    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["source"], "cache");
    assert!(json["total"].as_u64().unwrap() >= 1);
}

// ── E2E: Notifier integration ──

#[tokio::test(flavor = "multi_thread")]
async fn e2e_notifier_log_only() {
    use tools::notifier::{Notifier, PriceAlert};

    let notifier = Notifier::log_only();
    let alert = PriceAlert {
        subscription_id: "test-sub-1".to_string(),
        from_city: "北京".to_string(),
        to_city: "上海".to_string(),
        transport_type: "train".to_string(),
        current_price: 480.0,
        threshold: 500.0,
        message: "价格已降至目标以下".to_string(),
    };

    // Should not panic, just log
    notifier.send_alert(&alert).await;
}

// ── E2E: Metrics recording ──

#[test]
fn e2e_metrics_recorded() {
    tools::metrics::init_metrics();

    let mut executor = setup_executor();

    // Execute a few tools
    let _ = executor.execute(
        "wiki",
        r#"{"action":"remember","topic":"test","key":"k","value":"v"}"#,
    );
    let _ = executor.execute(
        "city_distance",
        r#"{"city":"北京","target_city":"上海"}"#,
    );
    let _ = executor.execute(
        "search_trains",
        "invalid json",
    );

    // Render Prometheus metrics
    let metrics_output = tools::metrics::render_metrics();
    assert!(
        metrics_output.contains("cctraveler_tool_calls_total"),
        "Should have tool call counter in metrics"
    );
    assert!(
        metrics_output.contains("cctraveler_tool_latency_seconds"),
        "Should have latency histogram in metrics"
    );
}

// ── E2E: Full search_hotels pipeline ──

#[test]
fn e2e_search_hotels_with_filters() {
    let mut executor = setup_executor();

    // Search for Beijing hotels with min star 4
    let result = executor.execute(
        "search_hotels",
        r#"{"city":"北京","min_star":4}"#,
    ).unwrap();
    // Should find the 4-star hotel we inserted
    assert!(result.contains("北京大饭店"));
}

// ── E2E: Complete tool spec validation ──

#[test]
fn e2e_all_tool_specs_valid() {
    let specs = tools::all_tool_specs();
    assert_eq!(specs.len(), 12, "Should have exactly 12 tools");

    for spec in &specs {
        assert!(!spec.name.is_empty(), "Tool name should not be empty");
        assert!(!spec.description.is_empty(), "Tool {} should have description", spec.name);
        assert!(
            !spec.input_schema.is_null(),
            "Tool {} should have input schema",
            spec.name
        );

        // input_schema is already a serde_json::Value, verify structure
        assert_eq!(
            spec.input_schema["type"], "object",
            "Tool {} schema should be of type 'object'",
            spec.name
        );
    }
}

// ── E2E: Error propagation ──

#[test]
fn e2e_error_propagation() {
    let mut executor = setup_executor();

    // Bad date format
    let result = executor.execute(
        "search_flights",
        r#"{"from_city":"北京","to_city":"上海","travel_date":"not-a-date"}"#,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{err}");
    assert!(
        err_msg.contains("日期") || err_msg.contains("date") || err_msg.contains("格式"),
        "Error should mention date format issue"
    );

    // Missing required field
    let result = executor.execute(
        "search_flights",
        r#"{"from_city":"北京"}"#,
    );
    assert!(result.is_err());

    // Empty JSON
    let result = executor.execute("wiki", "{}");
    assert!(result.is_err());
}
