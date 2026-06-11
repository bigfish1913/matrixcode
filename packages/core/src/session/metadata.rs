//! Session metadata types

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::compress::CompressionHistoryEntry;
use crate::providers::Message;

/// Session metadata stored in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier (UUID).
    pub id: String,
    /// User-defined session name (optional).
    pub name: Option<String>,
    /// Project path this session is associated with (optional).
    pub project_path: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
    /// Number of messages in the session.
    pub message_count: usize,
    /// Last input tokens reported.
    pub last_input_tokens: u64,
    /// Cumulative output tokens.
    pub total_output_tokens: u64,
    /// Compression history entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compression_history: Vec<CompressionHistoryEntry>,
}

impl SessionMetadata {
    /// Create a new session metadata with a fresh UUID and auto-generated name.
    pub fn new(project_path: Option<&Path>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            project_path: project_path.map(|p| p.to_string_lossy().to_string()),
            created_at: now,
            updated_at: now,
            message_count: 0,
            last_input_tokens: 0,
            total_output_tokens: 0,
            compression_history: Vec::new(),
        }
    }

    fn generate_time_name(time: DateTime<Utc>) -> String {
        let local: chrono::DateTime<chrono::Local> = time.with_timezone(&chrono::Local);
        local.format("%Y-%m-%d %H:%M").to_string()
    }

    pub fn add_compression_entry(&mut self, entry: CompressionHistoryEntry) {
        self.compression_history.push(entry);
        if self.compression_history.len() > 10 {
            self.compression_history.remove(0);
        }
    }

    pub fn total_tokens_saved(&self) -> u32 {
        self.compression_history
            .iter()
            .map(|e| e.tokens_saved)
            .sum()
    }

    pub fn compression_count(&self) -> usize {
        self.compression_history.len()
    }

    pub fn display_name(&self) -> String {
        if let Some(ref name) = self.name {
            name.clone()
        } else {
            Self::generate_time_name(self.created_at)
        }
    }

    pub fn short_id(&self) -> String {
        self.id.chars().take(8).collect()
    }

    pub fn format_line(&self, is_current: bool) -> String {
        let marker = if is_current { "*" } else { " " };
        let name = self.display_name();
        let msgs = self.message_count;
        let project = self
            .project_path
            .as_ref()
            .map(|p| {
                PathBuf::from(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone())
            })
            .unwrap_or_else(|| "-".to_string());

        let compression_info = if self.compression_count() > 0 {
            format!("  💾 {} comps", self.compression_count())
        } else {
            "".to_string()
        };

        format!(
            "{} {}  {} msgs  {}{}",
            marker, name, msgs, project, compression_info
        )
    }
}

/// Index of all sessions, stored in ~/.matrix/sessions/index.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionIndex {
    /// All known sessions.
    pub sessions: Vec<SessionMetadata>,
    /// ID of the most recently active session (for --continue).
    pub last_session_id: Option<String>,
}

impl SessionIndex {
    /// Find a session by ID or name.
    pub fn find(&self, query: &str) -> Option<&SessionMetadata> {
        if let Some(s) = self.sessions.iter().find(|s| s.id == query) {
            return Some(s);
        }
        if let Some(s) = self
            .sessions
            .iter()
            .find(|s| s.name.as_deref() == Some(query))
        {
            return Some(s);
        }
        if let Some(s) = self.sessions.iter().find(|s| s.id.starts_with(query)) {
            return Some(s);
        }
        None
    }

    pub fn last_session(&self) -> Option<&SessionMetadata> {
        self.last_session_id
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| s.id == *id))
    }

    pub fn upsert(&mut self, meta: SessionMetadata) {
        self.sessions.retain(|s| s.id != meta.id);
        self.sessions.push(meta.clone());
        self.last_session_id = Some(meta.id);
        self.sessions
            .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    pub fn remove(&mut self, id: &str) -> Option<SessionMetadata> {
        let removed = self.sessions.iter().position(|s| s.id == id);
        if let Some(idx) = removed {
            let meta = self.sessions.remove(idx);
            if self.last_session_id.as_deref() == Some(id) {
                self.last_session_id = self.sessions.first().map(|s| s.id.clone());
            }
            Some(meta)
        } else {
            None
        }
    }

    pub fn rename(&mut self, id: &str, new_name: &str) -> Result<()> {
        let session = self.sessions.iter_mut().find(|s| s.id == id);
        if let Some(s) = session {
            s.name = Some(new_name.to_string());
            s.updated_at = Utc::now();
            Ok(())
        } else {
            anyhow::bail!("session {} not found", id)
        }
    }
}

/// Message summary for display (lightweight version).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    pub role: String,
    pub preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    pub is_compressed: bool,
    pub original_index: usize,
}

impl MessageSummary {
    pub fn from_message(msg: &Message, index: usize) -> Self {
        use crate::providers::{ContentBlock, MessageContent, Role};
        use crate::truncate::truncate_chars;

        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
            Role::System => "system",
        };

        let preview = match &msg.content {
            MessageContent::Text(t) => truncate_chars(t, 100),
            MessageContent::Blocks(blocks) => {
                let parts: Vec<String> = blocks
                    .iter()
                    .take(3)
                    .map(|b| match b {
                        ContentBlock::Text { text } => truncate_chars(text, 50),
                        ContentBlock::ToolUse { name, .. } => format!("[{}]", name),
                        ContentBlock::ToolResult { content, .. } => truncate_chars(content, 50),
                        ContentBlock::Thinking { thinking, .. } => {
                            format!("💭 {}", truncate_chars(thinking, 30))
                        }
                        _ => "...".to_string(),
                    })
                    .collect();
                parts.join(" ")
            }
        };

        Self {
            role: role.to_string(),
            preview,
            timestamp: None,
            is_compressed: false,
            original_index: index,
        }
    }
}
