use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

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
}

impl SessionMetadata {
    /// Create a new session metadata with a fresh UUID.
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
        }
    }

    /// Get a display name for the session.
    /// Returns user-defined name if set, otherwise a truncated ID.
    pub fn display_name(&self) -> String {
        if let Some(ref name) = self.name {
            name.clone()
        } else {
            // Show first 8 characters of UUID as fallback
            format!("session-{}", &self.id[..8])
        }
    }

    /// Format the session for display in a list.
    pub fn format_line(&self, is_current: bool) -> String {
        let marker = if is_current { "*" } else { " " };
        let name = self.display_name();
        let time = self.updated_at.format("%Y-%m-%d %H:%M");
        let msgs = self.message_count;
        let project = self.project_path
            .as_ref()
            .map(|p| {
                // Show just the directory name, not full path
                PathBuf::from(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone())
            })
            .unwrap_or_else(|| "-".to_string());
        
        format!("{} {}  [{}]  {} msgs  {}", marker, name, time, msgs, project)
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
        if let Some(s) = self.sessions.iter().find(|s| s.name.as_deref() == Some(query)) {
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
        self.sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
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

/// Full session data including messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub metadata: SessionMetadata,
    pub messages: Vec<Message>,
}

impl Session {
    /// Create a new empty session.
    pub fn new(project_path: Option<&Path>) -> Self {
        Self {
            metadata: SessionMetadata::new(project_path),
            messages: Vec::new(),
        }
    }

    /// Create a session from existing messages.
    pub fn from_messages(messages: Vec<Message>, project_path: Option<&Path>) -> Self {
        let mut meta = SessionMetadata::new(project_path);
        meta.message_count = messages.len();
        Self {
            metadata: meta,
            messages,
        }
    }

    /// Update metadata after a turn.
    pub fn update_stats(&mut self, last_input_tokens: u32, total_output_tokens: u64) {
        self.metadata.message_count = self.messages.len();
        self.metadata.last_input_tokens = last_input_tokens as u64;
        self.metadata.total_output_tokens = total_output_tokens;
        self.metadata.updated_at = Utc::now();
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
        let json = serde_json::to_string_pretty(&self.index)
            .context("serializing session index")?;
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
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Continue the last session (for --continue).
    pub fn continue_last(&mut self, project_path: Option<&Path>) -> Result<Option<&Session>> {
        let last_id = self.index.last_session().map(|m| m.id.clone());
        if let Some(id) = last_id {
            self.load_session(&id)?;
            // Update project path if provided and different
            if let Some(path) = project_path {
                if let Some(ref mut session) = self.current_session {
                    session.metadata.project_path = Some(path.to_string_lossy().to_string());
                }
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
            if let Some(path) = project_path {
                if let Some(ref mut session) = self.current_session {
                    session.metadata.project_path = Some(path.to_string_lossy().to_string());
                }
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
        let session: Session = serde_json::from_str(&data)
            .with_context(|| format!("parsing session file {}", path.display()))?;
        self.current_session = Some(session);
        Ok(())
    }

    /// Save the current session to disk.
    pub fn save_current(&mut self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            let path = self.session_path(&session.metadata.id);
            let json = serde_json::to_string(session)
                .context("serializing session")?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, json)
                .with_context(|| format!("writing session tmp file {}", tmp.display()))?;
            std::fs::rename(&tmp, &path)
                .with_context(|| format!("renaming session tmp file to {}", path.display()))?;
            
            // Update index
            self.index.upsert(session.metadata.clone());
            self.save_index()?;
        }
        Ok(())
    }

    /// Update current session stats after a turn.
    pub fn update_stats(&mut self, last_input_tokens: u32, total_output_tokens: u64) {
        if let Some(ref mut session) = self.current_session {
            session.update_stats(last_input_tokens, total_output_tokens);
        }
    }

    /// Set messages for the current session.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        if let Some(ref mut session) = self.current_session {
            session.messages = messages;
            session.metadata.message_count = session.messages.len();
            session.metadata.updated_at = Utc::now();
        }
    }

    /// Get the current session's messages.
    pub fn messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.messages.as_slice())
    }

    /// Get mutable reference to messages.
    pub fn messages_mut(&mut self) -> Option<&mut Vec<Message>> {
        self.current_session.as_mut().map(|s| &mut s.messages)
    }

    /// Get the current session ID.
    pub fn current_id(&self) -> Option<&str> {
        self.current_session.as_ref().map(|s| s.metadata.id.as_str())
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