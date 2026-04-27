use anyhow::Result;
use api::AnthropicRuntimeClient;
use runtime::{ConversationRuntime, RuntimeConfig, SystemPromptBuilder};
use std::io::Read;
use std::path::Path;
use storage::Database;
use tools::TravelerToolExecutor;

/// 一次性模式：读取 prompt（来自 -p 参数或 stdin），跑一轮，打印结果后退出。
pub fn run(config: &RuntimeConfig, db_path: &Path, prompt: Option<String>) -> Result<()> {
    let user_input = match prompt {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    let user_input = user_input.trim();
    if user_input.is_empty() {
        anyhow::bail!("没有输入内容。请通过 -p \"...\" 或 stdin 提供 prompt。");
    }

    let api_key = config.agent.resolve_api_key().ok_or_else(|| {
        anyhow::anyhow!(
            "API key not found. Set api_key in config.toml [agent], or env ANTHROPIC_API_KEY."
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
    let tool_executor = TravelerToolExecutor::new(db, config.scraper.base_url.clone())
        .with_redis(redis);

    let system_prompt = SystemPromptBuilder::build_default();
    let mut rt = ConversationRuntime::new(
        api_client,
        tool_executor,
        config.agent.model.clone(),
        system_prompt,
        config.agent.max_turns as usize,
    );

    let summary = rt.run_turn(user_input)?;
    println!("{}", summary.assistant_text);
    Ok(())
}
