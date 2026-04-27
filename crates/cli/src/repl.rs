//! 交互式 REPL（聊天模式）。从原 main.rs 的 run_chat 抽出。

use crate::expand;
use crate::slash::{Action, Command};
use anyhow::Result;
use api::AnthropicRuntimeClient;
use runtime::{ConversationRuntime, RuntimeConfig, Session, SystemPromptBuilder};
use rustyline::DefaultEditor;
use std::path::{Path, PathBuf};
use storage::Database;
use tools::TravelerToolExecutor;

pub fn run(config: &RuntimeConfig, db_path: &Path, resume_id: Option<String>) -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║   CCTraveler AI 旅行助手               ║");
    println!("║   输入自然语言查询酒店/火车/机票/行程    ║");
    println!("║   /help 查看命令，/quit 退出            ║");
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
    rt.session.workspace_root = Some(cwd.clone());

    // 恢复历史 session（如指定）
    if let Some(id) = resume_id {
        match Session::load(&cwd, &id) {
            Ok(loaded) => {
                let n = loaded.messages.len();
                rt.session = loaded;
                rt.session.workspace_root = Some(cwd.clone());
                println!("✔ 已恢复 session {id}（{n} 条历史消息）\n");
            }
            Err(e) => {
                eprintln!("⚠ 恢复 session {id} 失败: {e}");
                eprintln!("  将开启新会话。\n");
            }
        }
    }

    // 工具调用实时状态行
    rt.set_tool_listener(std::sync::Arc::new(|ev: &runtime::ToolEvent| match ev {
        runtime::ToolEvent::Start { name, input } => {
            let summary = summarize_input(input);
            println!("  ⏳ {name}({summary})");
        }
        runtime::ToolEvent::Finish {
            name,
            ok,
            output_chars,
            elapsed_ms,
        } => {
            let icon = if *ok { "✓" } else { "✗" };
            println!("  {icon} {name}  ({elapsed_ms}ms, {output_chars} chars)");
        }
    }));

    // Token 流式输出：每段 text_delta 立刻打印 + flush
    rt.set_text_listener(std::sync::Arc::new(|delta: &str| {
        use std::io::Write;
        print!("{delta}");
        let _ = std::io::stdout().flush();
    }));

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

        // 斜杠命令：先于 LLM 处理
        if let Some(cmd) = Command::parse(input) {
            match cmd.execute(&mut rt, db_path)? {
                Action::Continue => {
                    println!();
                    continue;
                }
                Action::Quit => break,
            }
        }

        // 兼容老的裸 quit/exit/q（不带斜杠）
        if matches!(input, "quit" | "exit" | "q") {
            println!("再见！");
            break;
        }

        // 展开 @file / @url 引用
        let prepared = if input.contains('@') {
            expand::expand(input)
        } else {
            input.to_string()
        };

        // 流式：先打 assistant 前缀，让 text listener 把 token 接着流到同一行后面。
        {
            use std::io::Write;
            print!("\nassistant> ");
            let _ = std::io::stdout().flush();
        }

        match rt.run_turn(&prepared) {
            Ok(summary) => {
                println!();
                if summary.tool_calls_made > 0 {
                    println!(
                        "  [工具调用: {} 次 | tokens: {} in / {} out]",
                        summary.tool_calls_made, summary.input_tokens, summary.output_tokens
                    );
                }
                println!();
            }
            Err(e) => {
                eprintln!("\nError: {e}");
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

/// 找到 `.cctraveler/sessions/` 下 mtime 最新的 session_id；无则 None。
pub fn find_latest_session() -> Option<String> {
    let dir = std::path::PathBuf::from(".cctraveler/sessions");
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match &best {
            Some((t, _)) if *t >= mtime => {}
            _ => best = Some((mtime, stem.to_string())),
        }
    }
    best.map(|(_, id)| id)
}

/// 把 tool 入参渲染成单行简短摘要（最多 60 字符）。
fn summarize_input(input: &serde_json::Value) -> String {
    let s = match input {
        serde_json::Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(4)
                .map(|(k, v)| format!("{k}={}", short_value(v)))
                .collect();
            parts.join(", ")
        }
        other => short_value(other),
    };
    if s.chars().count() > 60 {
        let truncated: String = s.chars().take(58).collect();
        format!("{truncated}..")
    } else {
        s
    }
}

fn short_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            if s.chars().count() > 20 {
                let truncated: String = s.chars().take(18).collect();
                format!("\"{truncated}..\"")
            } else {
                format!("\"{s}\"")
            }
        }
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(a) => format!("[{}]", a.len()),
        serde_json::Value::Object(o) => format!("{{{}}}", o.len()),
    }
}
