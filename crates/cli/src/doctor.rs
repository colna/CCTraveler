use anyhow::Result;
use runtime::RuntimeConfig;
use std::path::Path;
use storage::Database;

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    println!("CCTraveler 环境诊断");
    println!("───────────────────");

    // 1. 配置加载
    let cfg = match RuntimeConfig::load_layered(config_path) {
        Ok(c) => {
            println!("✔ 配置加载成功");
            Some(c)
        }
        Err(e) => {
            println!("✗ 配置加载失败: {e}");
            None
        }
    };

    let mut all_ok = cfg.is_some();

    // 2. API key
    if let Some(c) = &cfg {
        match c.agent.resolve_api_key() {
            Some(k) if !k.is_empty() => {
                println!("✔ API key 已配置 (长度 {})", k.len());
            }
            _ => {
                println!("✗ 未找到 API key (config.toml [agent].api_key 或 ANTHROPIC_API_KEY)");
                all_ok = false;
            }
        }
        println!("  base_url: {}", c.agent.resolve_base_url());
        println!("  model:    {}", c.agent.model);
    }

    // 3. SQLite 可写
    if let Some(c) = &cfg {
        let db_path = std::path::PathBuf::from(&c.storage.db_path);
        if let Some(parent) = db_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("✗ 数据目录无法创建: {} ({e})", parent.display());
                all_ok = false;
            }
        }
        match Database::open(&db_path) {
            Ok(_) => println!("✔ SQLite 可写: {}", db_path.display()),
            Err(e) => {
                println!("✗ SQLite 打开失败: {e}");
                all_ok = false;
            }
        }
    }

    // 4. Scraper 服务
    if let Some(c) = &cfg {
        let url = format!("{}/healthz", c.scraper.base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()?;
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                println!("✔ Scraper 服务可达: {}", c.scraper.base_url);
            }
            Ok(r) => {
                println!(
                    "⚠ Scraper 响应异常 ({}): {}",
                    r.status(),
                    c.scraper.base_url
                );
            }
            Err(e) => {
                println!(
                    "⚠ Scraper 不可达 ({}): {e}\n  部分工具(scrape/search_*)将不可用",
                    c.scraper.base_url
                );
            }
        }
    }

    // 5. Redis (可选)
    if let Some(c) = &cfg {
        if c.redis.enabled {
            println!("  Redis 已启用: {}", c.redis.url);
        } else {
            println!("  Redis 未启用 (可选)");
        }
    }

    println!("───────────────────");
    if all_ok {
        println!("✔ 核心检查通过");
        Ok(())
    } else {
        anyhow::bail!("部分检查未通过，请按上方提示修复")
    }
}
