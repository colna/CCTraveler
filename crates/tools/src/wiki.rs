use runtime::types::RuntimeError;
use serde::Deserialize;
use storage::Database;
use tracing::info;

#[derive(Deserialize)]
struct WikiParams {
    action: String,
    topic: Option<String>,
    key: Option<String>,
    value: Option<String>,
    metadata: Option<String>,
}

pub fn handle_wiki(db: &Database, input: &str) -> Result<String, RuntimeError> {
    let params: WikiParams = serde_json::from_str(input).map_err(|e| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: format!("Invalid input: {e}"),
    })?;

    match params.action.as_str() {
        "remember" => handle_remember(db, &params),
        "recall" => handle_recall(db, &params),
        "list" => handle_list(db, &params),
        "forget" => handle_forget(db, &params),
        other => Err(RuntimeError::Tool {
            tool_name: "wiki".into(),
            message: format!("Unknown action: {other}. Use remember/recall/list/forget."),
        }),
    }
}

fn handle_remember(db: &Database, params: &WikiParams) -> Result<String, RuntimeError> {
    let topic = params.topic.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "remember requires topic".into(),
    })?;
    let key = params.key.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "remember requires key".into(),
    })?;
    let value = params.value.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "remember requires value".into(),
    })?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    db.upsert_wiki_entry(&id, topic, key, value, params.metadata.as_deref(), &now)
        .map_err(|e| RuntimeError::Tool {
            tool_name: "wiki".into(),
            message: e.to_string(),
        })?;

    info!("Wiki: remembered [{topic}] {key}");
    Ok(format!("已记住: [{topic}] {key} = {value}"))
}

fn handle_recall(db: &Database, params: &WikiParams) -> Result<String, RuntimeError> {
    let topic = params.topic.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "recall requires topic".into(),
    })?;
    let key = params.key.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "recall requires key".into(),
    })?;

    let entry = db.get_wiki_entry(topic, key).map_err(|e| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: e.to_string(),
    })?;

    match entry {
        Some(e) => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "topic": e.topic,
            "key": e.key,
            "value": e.value,
            "metadata": e.metadata,
            "updated_at": e.updated_at,
        }))
        .unwrap_or_default()),
        None => Ok(format!("未找到 [{topic}] {key} 的记录。")),
    }
}

fn handle_list(db: &Database, params: &WikiParams) -> Result<String, RuntimeError> {
    let entries = db
        .list_wiki_entries(params.topic.as_deref())
        .map_err(|e| RuntimeError::Tool {
            tool_name: "wiki".into(),
            message: e.to_string(),
        })?;

    if entries.is_empty() {
        return Ok("知识库中暂无记录。".to_string());
    }

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "topic": e.topic,
                "key": e.key,
                "value": e.value,
                "updated_at": e.updated_at,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "total": entries.len(),
        "entries": items
    }))
    .unwrap_or_default())
}

fn handle_forget(db: &Database, params: &WikiParams) -> Result<String, RuntimeError> {
    let topic = params.topic.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "forget requires topic".into(),
    })?;
    let key = params.key.as_deref().ok_or_else(|| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: "forget requires key".into(),
    })?;

    db.delete_wiki_entry(topic, key).map_err(|e| RuntimeError::Tool {
        tool_name: "wiki".into(),
        message: e.to_string(),
    })?;

    info!("Wiki: forgot [{topic}] {key}");
    Ok(format!("已删除: [{topic}] {key}"))
}
