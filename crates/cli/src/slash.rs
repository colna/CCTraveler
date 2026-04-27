//! 斜杠命令解析与执行。
//!
//! 设计：解析阶段返回 `Command` 枚举，执行阶段拿到 `&mut ConversationRuntime`
//! 完成副作用（清空、改模型、导出等）。`Action` 表示执行后 REPL 应做什么
//! （继续/退出/吞掉这次输入）。

use anyhow::Result;
use api::AnthropicRuntimeClient;
use runtime::types::ContentBlock;
use runtime::ConversationRuntime;
use std::collections::HashMap;
use std::path::Path;
use tools::TravelerToolExecutor;

pub type Runtime = ConversationRuntime<AnthropicRuntimeClient, TravelerToolExecutor>;

/// 执行结果。
pub enum Action {
    /// 命令已处理，继续 REPL 循环。
    Continue,
    /// 命令请求退出。
    Quit,
}

#[derive(Debug)]
pub enum Command {
    Help,
    Clear,
    New,
    Cost,
    Tools,
    Model(Option<String>),
    Sessions,
    Export(String),
    Quit,
    Unknown(String),
}

impl Command {
    /// 输入若以 `/` 开头则解析为 Command，否则返回 None。
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let line = input.trim();
        if !line.starts_with('/') {
            return None;
        }
        let mut parts = line[1..].splitn(2, char::is_whitespace);
        let head = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();
        let cmd = match head {
            "help" | "h" | "?" => Self::Help,
            "clear" => Self::Clear,
            "new" => Self::New,
            "cost" => Self::Cost,
            "tools" => Self::Tools,
            "model" => Self::Model((!rest.is_empty()).then(|| rest.to_string())),
            "sessions" => Self::Sessions,
            "export" => Self::Export(if rest.is_empty() {
                "md".into()
            } else {
                rest.to_string()
            }),
            "quit" | "exit" | "q" => Self::Quit,
            other => Self::Unknown(other.to_string()),
        };
        Some(cmd)
    }

    pub fn execute(self, rt: &mut Runtime, db_path: &Path) -> Result<Action> {
        match self {
            Self::Help => print_help(),
            Self::Clear | Self::New => {
                rt.reset_session();
                println!("✔ 已开启新会话");
            }
            Self::Cost => print_cost(rt),
            Self::Tools => print_tools(rt),
            Self::Model(None) => println!("当前模型: {}", rt.model()),
            Self::Model(Some(name)) => {
                rt.set_model(name.clone());
                println!("✔ 已切换模型为 {name}");
            }
            Self::Sessions => print_sessions(db_path),
            Self::Export(format) => export_session(rt, &format)?,
            Self::Quit => {
                println!("再见！");
                return Ok(Action::Quit);
            }
            Self::Unknown(name) => {
                println!("未知命令: /{name}（输入 /help 查看可用命令）");
            }
        }
        Ok(Action::Continue)
    }
}

fn print_help() {
    println!(
        "可用斜杠命令：\n\
         \x20 /help, /h, /?       显示本帮助\n\
         \x20 /clear, /new        清空当前会话上下文\n\
         \x20 /cost               查看 token 用量\n\
         \x20 /tools              列出已加载工具\n\
         \x20 /model [name]       查看或切换模型\n\
         \x20 /sessions           列出历史 session\n\
         \x20 /export [md|json]   导出当前会话\n\
         \x20 /quit, /exit, /q    退出"
    );
}

fn print_cost(rt: &Runtime) {
    let u = rt.usage();
    let input = u.total_input_tokens;
    let output = u.total_output_tokens;
    // Anthropic Sonnet 4 参考价格：input $3 / output $15 per 1M (估算)
    let cost = (f64::from(input) * 3.0 + f64::from(output) * 15.0) / 1_000_000.0;
    println!(
        "本次会话累计：\n\
         \x20 input tokens : {input}\n\
         \x20 output tokens: {output}\n\
         \x20 估算成本     : ${cost:.4} (按 sonnet 价 input $3/M, output $15/M)\n\
         \x20 模型         : {model}",
        model = rt.model()
    );
}

fn print_tools(rt: &Runtime) {
    let specs = rt.tools();
    println!("已加载 {} 个工具：", specs.len());
    for s in specs {
        let desc = s.description.lines().next().unwrap_or("");
        let desc = if desc.chars().count() > 60 {
            let truncated: String = desc.chars().take(58).collect();
            format!("{truncated}..")
        } else {
            desc.to_string()
        };
        println!("  · {:<22} {}", s.name, desc);
    }
}

fn print_sessions(_db_path: &Path) {
    let dir = std::path::PathBuf::from(".cctraveler/sessions");
    if !dir.exists() {
        println!("(无历史 session — .cctraveler/sessions/ 不存在)");
        return;
    }
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "jsonl")
        })
        .collect();
    if entries.is_empty() {
        println!("(无历史 session)");
        return;
    }
    let mut rows: Vec<(String, std::time::SystemTime, u64)> = entries
        .iter()
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let meta = e.metadata().ok()?;
            Some((stem, meta.modified().ok()?, meta.len()))
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    println!("历史 session（按修改时间倒序）：");
    for (id, mtime, size) in rows.iter().take(20) {
        let dt: chrono::DateTime<chrono::Local> = (*mtime).into();
        println!(
            "  · {:<32} {} {:>6} KB",
            id,
            dt.format("%Y-%m-%d %H:%M"),
            size / 1024
        );
    }
    println!("\n使用 `cctraveler --resume <id>` 或 `cctraveler -c` 恢复会话");
}

fn export_session(rt: &Runtime, format: &str) -> Result<()> {
    let path = std::path::PathBuf::from(format!(
        ".cctraveler/exports/{}.{}",
        rt.session.session_id,
        if format == "json" { "json" } else { "md" }
    ));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = match format {
        "json" => serde_json::to_string_pretty(&rt.session)?,
        _ => session_to_markdown(rt),
    };
    std::fs::write(&path, content)?;
    println!("✔ 已导出到 {}", path.display());
    Ok(())
}

fn session_to_markdown(rt: &Runtime) -> String {
    let mut tool_uses: HashMap<String, (String, String)> = HashMap::new(); // id -> (name, input)
    let mut out = String::new();
    out.push_str(&format!("# CCTraveler Session `{}`\n\n", rt.session.session_id));
    out.push_str(&format!("- model: `{}`\n", rt.model()));
    let u = rt.usage();
    out.push_str(&format!(
        "- tokens: {} in / {} out\n\n---\n\n",
        u.total_input_tokens, u.total_output_tokens
    ));

    for msg in &rt.session.messages {
        let role = match msg.role {
            runtime::types::MessageRole::User => "User",
            runtime::types::MessageRole::Assistant => "Assistant",
        };
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    out.push_str(&format!("**{role}:** {text}\n\n"));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.insert(
                        id.clone(),
                        (name.clone(), input.to_string()),
                    );
                    out.push_str(&format!(
                        "**{role}** → 调用 `{name}`：\n```json\n{}\n```\n\n",
                        serde_json::to_string_pretty(input).unwrap_or_default()
                    ));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    output,
                    is_error,
                } => {
                    let name = tool_uses
                        .get(tool_use_id)
                        .map_or("?", |(n, _)| n.as_str());
                    let tag = if *is_error { "✗" } else { "✓" };
                    let preview: String = output.chars().take(400).collect();
                    out.push_str(&format!(
                        "**Tool {tag} {name}:**\n```\n{preview}\n```\n\n"
                    ));
                }
            }
        }
    }
    out
}
