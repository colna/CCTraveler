use storage::Database;
use tools::geo::handle_query_city_info;

fn main() -> anyhow::Result<()> {
    // 打开数据库
    let db = Database::open(std::path::Path::new("data/cctraveler.db"))?;

    println!("=== 测试 1: 查询遵义概况 ===");
    let input = r#"{"city": "遵义", "info_type": "overview"}"#;
    let result = handle_query_city_info(&db, input)?;
    println!("{}\n", result);

    println!("=== 测试 2: 查询遵义区域 ===");
    let input = r#"{"city": "遵义", "info_type": "districts"}"#;
    let result = handle_query_city_info(&db, input)?;
    println!("{}\n", result);

    println!("=== 测试 3: 查询遵义景点 ===");
    let input = r#"{"city": "遵义", "info_type": "attractions"}"#;
    let result = handle_query_city_info(&db, input)?;
    println!("{}\n", result);

    println!("=== 测试 4: 查询遵义交通 ===");
    let input = r#"{"city": "遵义", "info_type": "transport"}"#;
    let result = handle_query_city_info(&db, input)?;
    println!("{}\n", result);

    println!("=== 测试 5: 查询北京概况 ===");
    let input = r#"{"city": "北京", "info_type": "overview"}"#;
    let result = handle_query_city_info(&db, input)?;
    println!("{}\n", result);

    Ok(())
}
