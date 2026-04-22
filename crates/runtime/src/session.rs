use crate::types::{ConversationMessage, Session, SessionCompaction, SessionPromptEntry};
use anyhow::Result;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// JSONL record types for session persistence.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
enum SessionRecord {
    #[serde(rename = "session_meta")]
    Meta {
        session_id: String,
        model: Option<String>,
        version: u32,
    },
    #[serde(rename = "message")]
    Message {
        message: ConversationMessage,
    },
    #[serde(rename = "compaction")]
    Compaction {
        compaction: SessionCompaction,
    },
}

const MAX_FILE_SIZE: u64 = 256 * 1024; // 256KB rotation threshold
const MAX_ROTATED_FILES: usize = 3;

impl Session {
    #[must_use] 
    pub fn new(model: Option<String>) -> Self {
        let session_id = format!("session-{}", chrono::Utc::now().timestamp_millis());
        Self {
            session_id,
            messages: Vec::new(),
            compaction: None,
            workspace_root: None,
            prompt_history: Vec::new(),
            model,
        }
    }

    pub fn push_message(&mut self, msg: ConversationMessage) {
        self.messages.push(msg);
    }

    pub fn push_user_prompt(&mut self, text: &str) {
        self.prompt_history.push(SessionPromptEntry {
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Session directory: <workspace>/.cctraveler/sessions/
    fn session_dir(workspace: &Path) -> PathBuf {
        workspace.join(".cctraveler").join("sessions")
    }

    fn session_file(workspace: &Path, session_id: &str) -> PathBuf {
        Self::session_dir(workspace).join(format!("{session_id}.jsonl"))
    }

    /// Save session to JSONL file (incremental append).
    pub fn save(&self, workspace: &Path) -> Result<()> {
        let dir = Self::session_dir(workspace);
        fs::create_dir_all(&dir)?;

        let path = Self::session_file(workspace, &self.session_id);

        // Check if rotation is needed
        if path.exists() {
            let metadata = fs::metadata(&path)?;
            if metadata.len() > MAX_FILE_SIZE {
                self.rotate_file(&path)?;
            }
        }

        // Full write: meta + all messages (atomic via temp file + rename)
        let tmp_path = path.with_extension("jsonl.tmp");
        {
            let mut file = fs::File::create(&tmp_path)?;

            // Write meta record
            let meta = SessionRecord::Meta {
                session_id: self.session_id.clone(),
                model: self.model.clone(),
                version: 1,
            };
            serde_json::to_writer(&mut file, &meta)?;
            writeln!(file)?;

            // Write compaction if present
            if let Some(compaction) = &self.compaction {
                let rec = SessionRecord::Compaction {
                    compaction: compaction.clone(),
                };
                serde_json::to_writer(&mut file, &rec)?;
                writeln!(file)?;
            }

            // Write all messages
            for msg in &self.messages {
                let rec = SessionRecord::Message {
                    message: msg.clone(),
                };
                serde_json::to_writer(&mut file, &rec)?;
                writeln!(file)?;
            }
        }

        // Atomic rename
        fs::rename(&tmp_path, &path)?;
        info!("Session saved: {}", path.display());
        Ok(())
    }

    /// Load session from JSONL file.
    pub fn load(workspace: &Path, session_id: &str) -> Result<Self> {
        let path = Self::session_file(workspace, session_id);
        if !path.exists() {
            anyhow::bail!("Session file not found: {}", path.display());
        }

        let file = fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);

        let mut session = Session {
            session_id: session_id.to_string(),
            messages: Vec::new(),
            compaction: None,
            workspace_root: Some(workspace.to_path_buf()),
            prompt_history: Vec::new(),
            model: None,
        };

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionRecord>(&line) {
                Ok(SessionRecord::Meta {
                    session_id: _,
                    model,
                    version: _,
                }) => {
                    session.model = model;
                }
                Ok(SessionRecord::Message { message }) => {
                    session.messages.push(message);
                }
                Ok(SessionRecord::Compaction { compaction }) => {
                    session.compaction = Some(compaction);
                }
                Err(e) => {
                    warn!("Skipping malformed JSONL line: {e}");
                }
            }
        }

        info!(
            "Loaded session '{}' with {} messages",
            session_id,
            session.messages.len()
        );
        Ok(session)
    }

    /// Rotate the session file when it exceeds the size threshold.
    fn rotate_file(&self, path: &Path) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
        let rotated = path.with_extension(format!("rot-{timestamp}.jsonl"));
        fs::rename(path, &rotated)?;
        info!("Rotated session file to {}", rotated.display());

        // Clean up old rotated files (keep MAX_ROTATED_FILES)
        let dir = path.parent().unwrap();
        let prefix = path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut rotated_files: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.to_string_lossy().contains(&format!("{prefix}.rot-"))
            })
            .collect();
        rotated_files.sort();
        while rotated_files.len() > MAX_ROTATED_FILES {
            if let Some(old) = rotated_files.first() {
                fs::remove_file(old)?;
                info!("Removed old rotated file: {}", old.display());
            }
            rotated_files.remove(0);
        }

        Ok(())
    }

    /// Estimate total input tokens (rough: 4 chars ≈ 1 token).
    #[must_use] 
    pub fn estimate_input_tokens(&self) -> u32 {
        let total_chars: usize = self
            .messages
            .iter()
            .map(|m| {
                m.content
                    .iter()
                    .map(|b| match b {
                        crate::types::ContentBlock::Text { text } => text.len(),
                        crate::types::ContentBlock::ToolUse { input, .. } => {
                            input.to_string().len()
                        }
                        crate::types::ContentBlock::ToolResult { output, .. } => output.len(),
                    })
                    .sum::<usize>()
            })
            .sum();
        (total_chars / 4) as u32
    }
}
