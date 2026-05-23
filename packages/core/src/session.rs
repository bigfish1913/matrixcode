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
            name: None, // Will be auto-generated from first meaningful user message
            project_path: project_path.map(|p| p.to_string_lossy().to_string()),
            created_at: now,
            updated_at: now,
            message_count: 0,
            last_input_tokens: 0,
            total_output_tokens: 0,
            compression_history: Vec::new(),
        }
    }

    /// Generate a friendly time-based name for the session.
    /// Format: "YYYY-MM-DD HH:mm" (e.g., "2024-01-15 14:30")
    fn generate_time_name(time: DateTime<Utc>) -> String {
        // Use local timezone for display
        let local: chrono::DateTime<chrono::Local> = time.with_timezone(&chrono::Local);
        local.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Add a compression entry to history.
    pub fn add_compression_entry(&mut self, entry: CompressionHistoryEntry) {
        self.compression_history.push(entry);
        // Keep only last 10 entries to avoid bloat
        if self.compression_history.len() > 10 {
            self.compression_history.remove(0);
        }
    }

    /// Get total tokens saved across all compressions.
    pub fn total_tokens_saved(&self) -> u32 {
        self.compression_history
            .iter()
            .map(|e| e.tokens_saved)
            .sum()
    }

    /// Get compression count.
    pub fn compression_count(&self) -> usize {
        self.compression_history.len()
    }

    /// Get a display name for the session.
    /// Returns user-defined name if set, otherwise a time-based fallback.
    pub fn display_name(&self) -> String {
        if let Some(ref name) = self.name {
            name.clone()
        } else {
            // Fallback: show creation time
            Self::generate_time_name(self.created_at)
        }
    }

    /// Get a short ID for the session (first 8 chars of UUID).
    pub fn short_id(&self) -> String {
        self.id[..8].to_string()
    }

    /// Format the session for display in a list.
    pub fn format_line(&self, is_current: bool) -> String {
        let marker = if is_current { "*" } else { " " };
        let name = self.display_name();
        let msgs = self.message_count;
        let project = self
            .project_path
            .as_ref()
            .map(|p| {
                // Show just the directory name, not full path
                PathBuf::from(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone())
            })
            .unwrap_or_else(|| "-".to_string());

        // Add compression info if any
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
        // First try exact ID match
        if let Some(s) = self.sessions.iter().find(|s| s.id == query) {
            return Some(s);
        }
        // Then try exact name match
        if let Some(s) = self
            .sessions
            .iter()
            .find(|s| s.name.as_deref() == Some(query))
        {
            return Some(s);
        }
        // Then try partial ID match (for convenience)
        if let Some(s) = self.sessions.iter().find(|s| s.id.starts_with(query)) {
            return Some(s);
        }
        None
    }

    /// Get the last session (for --continue).
    pub fn last_session(&self) -> Option<&SessionMetadata> {
        self.last_session_id
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| s.id == *id))
    }

    /// Add or update a session in the index.
    pub fn upsert(&mut self, meta: SessionMetadata) {
        // Remove existing entry with same ID
        self.sessions.retain(|s| s.id != meta.id);
        // Add new entry
        self.sessions.push(meta.clone());
        // Update last_session_id
        self.last_session_id = Some(meta.id);
        // Sort by updated_at descending (most recent first)
        self.sessions
            .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    /// Remove a session from the index.
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

    /// Rename a session.
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
/// Used when full message content is compressed but user still needs to see history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    /// Role of the message sender.
    pub role: String,
    /// Brief preview of content (truncated).
    pub preview: String,
    /// Timestamp (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    /// Whether this message was compressed.
    pub is_compressed: bool,
    /// Original message index before compression.
    pub original_index: usize,
}

impl MessageSummary {
    /// Create a summary from a message.
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
                        ContentBlock::Thinking { thinking, .. } => format!("💭 {}", truncate_chars(thinking, 30)),
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

/// Full session data including messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub metadata: SessionMetadata,
    /// Full message history for display (TUI shows this).
    #[serde(default)]
    pub full_messages: Vec<Message>,
    /// Compressed messages for API requests (Agent uses this).
    /// If empty, use full_messages (no compression happened).
    #[serde(default)]
    pub compressed_messages: Vec<Message>,
    /// Summaries of compressed messages (for TUI history view).
    #[serde(default)]
    pub message_summaries: Vec<MessageSummary>,
    /// Legacy field - migrated to full_messages on load.
    #[serde(default, skip_serializing)]
    pub messages: Vec<Message>,
}

impl Session {
    /// Create a new empty session.
    pub fn new(project_path: Option<&Path>) -> Self {
        Self {
            metadata: SessionMetadata::new(project_path),
            full_messages: Vec::new(),
            compressed_messages: Vec::new(),
            message_summaries: Vec::new(),
            messages: Vec::new(),
        }
    }

    /// Create a session from existing messages.
    pub fn from_messages(messages: Vec<Message>, project_path: Option<&Path>) -> Self {
        let mut meta = SessionMetadata::new(project_path);
        meta.message_count = messages.len();
        Self {
            metadata: meta,
            full_messages: messages.clone(),
            compressed_messages: Vec::new(),
            message_summaries: messages.iter().enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect(),
            messages: messages,
        }
    }

    /// Get messages for API requests (use compressed if available).
    pub fn api_messages(&self) -> &[Message] {
        if self.compressed_messages.is_empty() {
            &self.full_messages
        } else {
            &self.compressed_messages
        }
    }

    /// Get messages for display (always full messages).
    pub fn display_messages(&self) -> &[Message] {
        &self.full_messages
    }

    /// Update metadata after a turn.
    pub fn update_stats(&mut self, last_input_tokens: u32, total_output_tokens: u64) {
        self.metadata.message_count = self.full_messages.len();
        self.metadata.last_input_tokens = last_input_tokens as u64;
        self.metadata.total_output_tokens = total_output_tokens;
        self.metadata.updated_at = Utc::now();
    }

    /// Set compressed messages (called after compression).
    pub fn set_compressed(&mut self, compressed: Vec<Message>, summaries: Vec<MessageSummary>) {
        self.compressed_messages = compressed;
        self.message_summaries = summaries;
    }

    /// Migrate legacy messages field to full_messages.
    fn migrate_legacy(&mut self) {
        if !self.messages.is_empty() && self.full_messages.is_empty() {
            self.full_messages = self.messages.clone();
            self.message_summaries = self.messages.iter().enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect();
            self.messages.clear();
        }
    }
}

/// Manager for session storage.
pub struct SessionManager {
    /// Base directory for session storage (~/.matrix).
    base_dir: PathBuf,
    /// Current active session (if any).
    current_session: Option<Session>,
    /// Session index.
    index: SessionIndex,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Result<Self> {
        let base_dir = Self::get_base_dir()?;
        let manager = Self {
            base_dir,
            current_session: None,
            index: SessionIndex::default(),
        };
        manager.ensure_dirs()?;
        let mut manager = manager;
        manager.load_index()?;
        Ok(manager)
    }

    /// Get the base directory for session storage.
    fn get_base_dir() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE environment variable not set"))?;
        let mut p = PathBuf::from(home);
        p.push(".matrix");
        Ok(p)
    }

    /// Get the sessions directory.
    fn sessions_dir(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    /// Get the index file path.
    fn index_path(&self) -> PathBuf {
        self.sessions_dir().join("index.json")
    }

    /// Get the path for a specific session file.
    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir().join(format!("{}.json", id))
    }

    /// Ensure directories exist.
    fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("creating base dir {}", self.base_dir.display()))?;
        std::fs::create_dir_all(self.sessions_dir())
            .with_context(|| format!("creating sessions dir {}", self.sessions_dir().display()))?;
        Ok(())
    }

    /// Load the session index from disk.
    fn load_index(&mut self) -> Result<()> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading index file {}", path.display()))?;
        if data.trim().is_empty() {
            return Ok(());
        }
        self.index = serde_json::from_str(&data)
            .with_context(|| format!("parsing index file {}", path.display()))?;
        Ok(())
    }

    /// Save the session index to disk.
    fn save_index(&self) -> Result<()> {
        let path = self.index_path();
        let json =
            serde_json::to_string_pretty(&self.index).context("serializing session index")?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .with_context(|| format!("writing index tmp file {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("renaming index tmp file to {}", path.display()))?;
        Ok(())
    }

    /// Start a new session.
    pub fn start_new(&mut self, project_path: Option<&Path>) -> Result<&Session> {
        let session = Session::new(project_path);
        self.current_session = Some(session);
        self.save_current()?;
        // SAFETY: current_session was just set and save_current succeeded
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Continue the last session (for --continue).
    pub fn continue_last(&mut self, project_path: Option<&Path>) -> Result<Option<&Session>> {
        let last_id = self.index.last_session().map(|m| m.id.clone());
        if let Some(id) = last_id {
            self.load_session(&id)?;
            // Update project path if provided and different
            if let Some(path) = project_path
                && let Some(ref mut session) = self.current_session
            {
                session.metadata.project_path = Some(path.to_string_lossy().to_string());
            }
            Ok(self.current_session.as_ref())
        } else {
            Ok(None)
        }
    }

    /// Resume a specific session by ID or name (for --resume).
    pub fn resume(&mut self, query: &str, project_path: Option<&Path>) -> Result<Option<&Session>> {
        let session_id = self.index.find(query).map(|m| m.id.clone());
        if let Some(id) = session_id {
            self.load_session(&id)?;
            // Update project path if provided
            if let Some(path) = project_path
                && let Some(ref mut session) = self.current_session
            {
                session.metadata.project_path = Some(path.to_string_lossy().to_string());
            }
            Ok(self.current_session.as_ref())
        } else {
            Ok(None)
        }
    }

    /// Load a session from disk by ID.
    fn load_session(&mut self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if !path.exists() {
            anyhow::bail!("session file {} not found", path.display());
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading session file {}", path.display()))?;
        let mut session: Session = serde_json::from_str(&data)
            .with_context(|| format!("parsing session file {}", path.display()))?;

        // Migrate legacy messages field to full_messages
        session.migrate_legacy();

        // If session name is null but index has a name, use index's name
        if session.metadata.name.is_none()
            && let Some(index_meta) = self.index.find(id)
        {
            session.metadata.name = index_meta.name.clone();
        }

        self.current_session = Some(session);
        Ok(())
    }

    /// Save the current session to disk.
    pub fn save_current(&mut self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            // Update index first (if index save fails, session file won't be updated)
            self.index.upsert(session.metadata.clone());
            self.save_index()?;

            // Now save session file
            let path = self.session_path(&session.metadata.id);
            let json = serde_json::to_string(session).context("serializing session")?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, json)
                .with_context(|| format!("writing session tmp file {}", tmp.display()))?;
            std::fs::rename(&tmp, &path)
                .with_context(|| format!("renaming session tmp file to {}", path.display()))?;
        }
        Ok(())
    }

    /// Update current session stats after a turn.
    pub fn update_stats(&mut self, last_input_tokens: u32, total_output_tokens: u64) {
        if let Some(ref mut session) = self.current_session {
            session.update_stats(last_input_tokens, total_output_tokens);
        }
    }

    /// Record a compression event in the session history.
    pub fn record_compression(&mut self, entry: crate::compress::CompressionHistoryEntry) {
        if let Some(ref mut session) = self.current_session {
            session.metadata.add_compression_entry(entry);
        }
    }

    /// Set messages for the current session.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        if let Some(ref mut session) = self.current_session {
            // Auto-generate name from first user message if name is None
            if session.metadata.name.is_none()
                && !messages.is_empty()
                && let Some(name) = Self::generate_name_from_messages(&messages)
            {
                session.metadata.name = Some(name);
            }

            // Update both full_messages and summaries
            session.full_messages = messages.clone();
            session.message_summaries = messages.iter().enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect();
            session.metadata.message_count = session.full_messages.len();
            session.metadata.updated_at = Utc::now();
        }
    }

    /// Set compressed messages for the current session.
    pub fn set_compressed_messages(&mut self, compressed: Vec<Message>) {
        if let Some(ref mut session) = self.current_session {
            // Mark all summaries as compressed first
            for summary in &mut session.message_summaries {
                summary.is_compressed = true;
            }

            // Then mark summaries as NOT compressed if their original message is in compressed version
            // Compare by role and content preview (since Message doesn't implement PartialEq)
            for compressed_msg in &compressed {
                for (idx, full_msg) in session.full_messages.iter().enumerate() {
                    // Simple comparison: same role and similar content
                    if session.message_summaries.get(idx).is_some() {
                        let same_role = compressed_msg.role == full_msg.role;
                        if same_role {
                            // Mark as not compressed
                            if let Some(summary) = session.message_summaries.get_mut(idx) {
                                summary.is_compressed = false;
                            }
                        }
                    }
                }
            }

            session.compressed_messages = compressed;
        }
    }

    /// Get messages for API requests (compressed if available).
    pub fn api_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.api_messages())
    }

    /// Get messages for display (always full messages).
    pub fn display_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.display_messages())
    }

    /// Generate a human-readable session name from the first user message.
    /// Takes the first meaningful user input and truncates it.
    fn generate_name_from_messages(messages: &[Message]) -> Option<String> {
        use crate::providers::{ContentBlock, MessageContent, Role};

        // Find first meaningful user message (skip very short/generic ones)
        let user_messages: Vec<&Message> =
            messages.iter().filter(|m| m.role == Role::User).collect();

        for msg in user_messages.iter().take(3) {
            let text = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            };

            let cleaned = text.trim().lines().next().unwrap_or("").trim();

            // Skip too short or generic messages
            if cleaned.len() < 5 || is_generic_message(cleaned) {
                continue;
            }

            // Truncate to reasonable length for display
            let name = if cleaned.chars().count() > 40 {
                let truncated: String = cleaned.chars().take(37).collect();
                format!("{}...", truncated)
            } else {
                cleaned.to_string()
            };

            return Some(name);
        }

        None
    }

    /// Get the current session's messages (for API - compressed if available).
    pub fn messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.api_messages())
    }

    /// Get mutable reference to messages (returns full_messages for editing).
    pub fn messages_mut(&mut self) -> Option<&mut Vec<Message>> {
        self.current_session.as_mut().map(|s| &mut s.full_messages)
    }

    /// Get full messages for display (TUI).
    pub fn full_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.display_messages())
    }

    /// Get the current session ID.
    pub fn current_id(&self) -> Option<&str> {
        self.current_session
            .as_ref()
            .map(|s| s.metadata.id.as_str())
    }

    /// Get the current session name.
    pub fn current_name(&self) -> Option<&str> {
        self.current_session.as_ref().and_then(|s| s.name())
    }

    /// Rename the current session.
    pub fn rename_current(&mut self, new_name: &str) -> Result<()> {
        if let Some(ref session) = self.current_session {
            let id = session.metadata.id.clone();
            self.index.rename(&id, new_name)?;
            if let Some(ref mut session) = self.current_session {
                session.metadata.name = Some(new_name.to_string());
            }
            self.save_current()?;
        }
        Ok(())
    }

    /// Clear the current session (start fresh).
    pub fn clear_current(&mut self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            // Remove session file
            let path = self.session_path(&session.metadata.id);
            let _ = std::fs::remove_file(&path);
            // Remove from index
            self.index.remove(&session.metadata.id);
            self.save_index()?;
        }
        self.current_session = None;
        Ok(())
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> &[SessionMetadata] {
        &self.index.sessions
    }

    /// Check if there's a current session.
    pub fn has_current(&self) -> bool {
        self.current_session.is_some()
    }

    /// Get current session metadata.
    pub fn current_metadata(&self) -> Option<&SessionMetadata> {
        self.current_session.as_ref().map(|s| &s.metadata)
    }

    /// Get the history file path (legacy compatibility).
    pub fn history_path(&self) -> PathBuf {
        self.base_dir.join("history.txt")
    }
}

impl Session {
    /// Get the session name (user-defined or fallback).
    pub fn name(&self) -> Option<&str> {
        self.metadata.name.as_deref()
    }
}

use anyhow::Context;

/// Check if a message is too generic to be a good session name.
fn is_generic_message(msg: &str) -> bool {
    let generic = [
        "继续", "好的", "ok", "yes", "no", "是", "否", "嗯", "对", "行", "可以", "好", "谢谢",
        "thanks", "hi", "hello", "你好", "开始", "start",
    ];
    generic.iter().any(|g| msg.eq_ignore_ascii_case(g))
}
