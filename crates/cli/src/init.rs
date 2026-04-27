use anyhow::{Context, Result};
use rustyline::DefaultEditor;
use runtime::config::user_config_path;
use std::path::PathBuf;

const TEMPLATE: &str = r#"# CCTraveler 用户配置 (~/.cctraveler/config.toml)
# 可被项目级 ./.cctraveler/config.toml 覆盖。

[agent]
model = "{MODEL}"
max_turns = 50
api_key = "{API_KEY}"
base_url = ""

[scraper]
base_url = "{SCRAPER_URL}"
timeout_secs = 120
max_retries = 3

[storage]
db_path = "{DB_PATH}"

[ctrip]
default_city = "558"
default_adults = 1
default_children = 0
request_delay_ms = 3000
max_concurrent = 3
proxy_pool = []

[redis]
enabled = false

[notification]
enabled = false
webhook_urls = []
"#;

pub fn run() -> Result<()> {
    println!("╭─ CCTraveler 初始化向导 ─────────────────────╮");
    println!("│ 我会帮你创建 ~/.cctraveler/config.toml      │");
    println!("╰─────────────────────────────────────────────╯");
    println!();

    let mut editor = DefaultEditor::new()?;

    let api_key = prompt(
        &mut editor,
        "Anthropic API key (留空则使用 ANTHROPIC_API_KEY 环境变量)",
        "",
    )?;
    let model = prompt(
        &mut editor,
        "��认模型 [claude-sonnet-4-20250514]",
        "claude-sonnet-4-20250514",
    )?;
    let scraper_url = prompt(
        &mut editor,
        "Scraper 服务地址 [http://localhost:8300]",
        "http://localhost:8300",
    )?;
    let db_path = default_db_path();
    let db_path_str = prompt(
        &mut editor,
        &format!("SQLite 数据库路径 [{}]", db_path.display()),
        &db_path.to_string_lossy(),
    )?;

    let target = user_config_path()
        .context("无法解析 HOME 环境变量")?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if target.exists() {
        let overwrite = prompt(
            &mut editor,
            &format!("{} 已存在，覆盖? [y/N]", target.display()),
            "n",
        )?;
        if !matches!(overwrite.to_lowercase().as_str(), "y" | "yes") {
            println!("已取消，未修改任何文件。");
            return Ok(());
        }
    }

    let content = TEMPLATE
        .replace("{MODEL}", &model)
        .replace("{API_KEY}", &api_key)
        .replace("{SCRAPER_URL}", &scraper_url)
        .replace("{DB_PATH}", &db_path_str);

    std::fs::write(&target, content)?;

    println!();
    println!("✔ 已写入 {}", target.display());
    if api_key.is_empty() {
        println!("  ⚠ API key 留空，记得设置 ANTHROPIC_API_KEY 环境变量。");
    }
    println!();
    println!("现在��以直接运行 `cctraveler` 进入对话。");
    Ok(())
}

fn prompt(editor: &mut DefaultEditor, label: &str, default: &str) -> Result<String> {
    let line = editor.readline(&format!("? {label}\n› "))?;
    let line = line.trim();
    if line.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(line.to_string())
    }
}

fn default_db_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".cctraveler")
            .join("data")
            .join("cctraveler.db")
    } else {
        PathBuf::from("data/cctraveler.db")
    }
}
