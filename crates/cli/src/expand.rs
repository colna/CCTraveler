//! 用户输入中 `@path` / `@url` 的展开。
//!
//! 规则：
//! - `@http://...` / `@https://...` —— 抓取 URL 内容（HTML 转文本最简化，仅取
//!   <body> 文本节点的近似）。失败则保留原 token 并打印警告。
//! - `@./file` / `@/abs/file` / `@~/file` / `@relative.txt` —— 读本地文件。
//! - 仅识别由空白或行首行尾包围的 token，避免破坏邮箱、`@username` 等。
//! - 单段最长 32 KiB，超出截断并附 `...[truncated]`。
//!
//! 展开后输出的格式（喂给 LLM 的）：
//!
//! ```
//! <ref src="path/to/file.txt">
//! ...内容...
//! </ref>
//! ```

use std::time::Duration;

const MAX_BYTES: usize = 32 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(8);

pub fn expand(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (i, segment) in split_with_separators(input).into_iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if let Some(token) = segment.strip_prefix('@') {
            if token.is_empty() {
                out.push_str(&segment);
                continue;
            }
            match resolve(token) {
                Ok(content) => {
                    out.push('\n');
                    out.push_str(&content);
                    out.push('\n');
                }
                Err(e) => {
                    eprintln!("  ⚠ 无法展开 @{token}: {e}");
                    out.push_str(&segment);
                }
            }
        } else {
            out.push_str(&segment);
        }
    }
    out
}

/// 按空白切分，但保留 token；不区分多个空白。
fn split_with_separators(input: &str) -> Vec<String> {
    input.split_whitespace().map(str::to_string).collect()
}

fn resolve(token: &str) -> Result<String, String> {
    if token.starts_with("http://") || token.starts_with("https://") {
        fetch_url(token)
    } else {
        read_file(token)
    }
}

fn read_file(path: &str) -> Result<String, String> {
    let expanded = if let Some(stripped) = path.strip_prefix("~/") {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(stripped))
            .map_err(|_| "HOME 未设置".to_string())?
    } else {
        std::path::PathBuf::from(path)
    };
    if !expanded.exists() {
        return Err(format!("文件不存在: {}", expanded.display()));
    }
    let raw = std::fs::read(&expanded).map_err(|e| format!("读取失败: {e}"))?;
    let truncated_note;
    let bytes = if raw.len() > MAX_BYTES {
        truncated_note = format!("\n...[truncated, original {} bytes]", raw.len());
        &raw[..MAX_BYTES]
    } else {
        truncated_note = String::new();
        &raw[..]
    };
    let text = String::from_utf8_lossy(bytes);
    Ok(format!(
        "<ref src=\"{}\">\n{text}{truncated_note}\n</ref>",
        expanded.display()
    ))
}

fn fetch_url(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent("cctraveler/0.1")
        .build()
        .map_err(|e| format!("构建 client 失败: {e}"))?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.text().map_err(|e| e.to_string())?;
    let text = strip_html(&body);
    let truncated_note;
    let slice = if text.len() > MAX_BYTES {
        truncated_note = format!("\n...[truncated, original {} bytes]", text.len());
        &text[..MAX_BYTES]
    } else {
        truncated_note = String::new();
        &text[..]
    };
    Ok(format!(
        "<ref src=\"{url}\">\n{slice}{truncated_note}\n</ref>"
    ))
}

/// 极简 HTML → 纯文本：去掉 `<...>` 标签 + 解码常见实体。
/// 不追求完美，只求"喂给 LLM 时去掉视觉噪音"。
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut buf = String::new();
    for ch in html.chars() {
        if in_script {
            buf.push(ch);
            if buf.ends_with("</script>") || buf.ends_with("</style>") {
                in_script = false;
                buf.clear();
            }
            continue;
        }
        if ch == '<' {
            in_tag = true;
            buf.clear();
            continue;
        }
        if in_tag {
            buf.push(ch);
            if ch == '>' {
                in_tag = false;
                let lower = buf.to_ascii_lowercase();
                if lower.starts_with("script") || lower.starts_with("style") {
                    in_script = true;
                }
                buf.clear();
            }
            continue;
        }
        out.push(ch);
    }
    let mut result = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // 折叠连续空白
    result = result.split_whitespace().collect::<Vec<_>>().join(" ");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_no_at() {
        assert_eq!(expand("hello world"), "hello world");
    }

    #[test]
    fn strip_html_basic() {
        assert_eq!(
            strip_html("<p>hello <b>world</b></p><script>x()</script>"),
            "hello world"
        );
    }

    #[test]
    fn unknown_file_keeps_token() {
        let out = expand("see @does/not/exist.txt please");
        assert!(out.contains("@does/not/exist.txt"));
    }
}
