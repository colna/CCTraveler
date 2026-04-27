//! 交互式 REPL（聊天模式）。从原 main.rs 的 run_chat 抽出。

use anyhow::Result;
use api::AnthropicRuntimeClient;
use runtime::{ConversationRuntime, RuntimeConfig, SystemPromptBuilder};
use rustyline::DefaultEditor;
use std::path::{Path, PathBuf};
use storage::Database;
use tools::TravelerToolExecutor;

pub fn run(config: &RuntimeConfig, db_path: &Path) -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║   CCTraveler AI 旅行助手               ║");
    println!("║   输入自然语言查询酒店/火车/机票/行程    ║");
    println!("║   输入 quit 或 exit 退出                ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    tools::metrics::init_metrics();

    let api_key = config.agent.resolve_api_key().ok_or_else(|| {
        anyhow::anyhow!(
            "API key not found.\n\
             Set api_key in config.toml [agent] section, or set ANTHROPIC_API_KEY env var.\n\
             或运行 `cctraveler init` 重新初始化配置。"
        )
    })?;
    let base_url = config.agent.resolve_base_url();
    let api_client = AnthropicRuntimeClient::with_base_url(api_key, base_url);

    let db = Database::open(db_path)?;
    let redis = tools::cache::RedisCache::new(
        config.redis.enabled,
        &config.redis.url,
        config.redis.ttl_seconds,
    );
    let tool_executor =
        TravelerToolExecutor::new(db, config.scraper.base_url.clone()).with_redis(redis);

    let mut scheduler = tools::scheduler::PriceScheduler::new(
        db_path.to_path_buf(),
        config.scraper.base_url.clone(),
        3600,
    );
    if config.notification.enabled && !config.notification.webhook_urls.is_empty() {
        scheduler = scheduler.with_webhooks(config.notification.webhook_urls.clone());
    }
    let _scheduler_handle = scheduler.spawn();

    let system_prompt = SystemPromptBuilder::build_default();

    let mut rt = ConversationRuntime::new(
        api_client,
        tool_executor,
        config.agent.model.clone(),
        system_prompt,
        config.agent.max_turns as usize,
    );

    let cwd = std::env::current_dir()?;
    rt.session.workspace_root = Some(cwd);

    let mut editor = DefaultEditor::new()?;
    let history_path = history_file();
    if let Some(ref path) = history_path {
        let _ = editor.load_history(path);
    }

    loop {
        let input = match editor.readline("you> ") {
            Ok(line) => line,
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("(Ctrl+C) 输入 quit 退出");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        let _ = editor.add_history_entry(input);

        if matches!(input, "quit" | "exit" | "q") {
            println!("再见！");
            break;
        }

        match rt.run_turn(input) {
            Ok(summary) => {
                println!("\nassistant> {}", summary.assistant_text);
                if summary.tool_calls_made > 0 {
                    println!(
                        "  [工具调用: {} 次 | tokens: {} in / {} out]",
                        summary.tool_calls_made, summary.input_tokens, summary.output_tokens
                    );
                }
                println!();
            }
            Err(e) => {
                eprintln!("Error: {e}");
                println!();
            }
        }
    }

    if let Err(e) = rt.save_session() {
        eprintln!("Warning: failed to save session: {e}");
    }
    if let Some(ref path) = history_path {
        let _ = editor.save_history(path);
    }
    Ok(())
}

fn history_file() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".cctraveler");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("history.txt"))
}
