//! Session manager for storage operations

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

use super::metadata::{SessionMetadata, SessionIndex, MessageSummary};
use super::session::{Session, SessionFileLock};
use crate::compress::CompressionHistoryEntry;
use crate::providers::{ContentBlock, Message, MessageContent, Role};

/// Check if a message is too generic to be a good session name.
fn is_generic_message(msg: &str) -> bool {
    let generic = [
        "继续", "好的", "ok", "yes", "no", "是", "否", "嗯", "对", "行", "可以", "好", "谢谢",
        "thanks", "hi", "hello", "你好", "开始", "start",
    ];
    generic.iter().any(|g| msg.eq_ignore_ascii_case(g))
}

/// Manager for session storage.
pub struct SessionManager {
    base_dir: PathBuf,
    current_session: Option<Session>,
    index: SessionIndex,
    lock: SessionFileLock,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Result<Self> {
        let base_dir = Self::get_base_dir()?;
        let lock = SessionFileLock::new(&base_dir);
        let manager = Self {
            base_dir,
            current_session: None,
            index: SessionIndex::default(),
            lock,
        };
        manager.ensure_dirs()?;
        let mut manager = manager;
        manager.load_index()?;
        Ok(manager)
    }

    fn get_base_dir() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE environment variable not set"))?;
        let mut p = PathBuf::from(home);
        p.push(".matrix");
        Ok(p)
    }

    fn sessions_dir(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    fn index_path(&self) -> PathBuf {
        self.sessions_dir().join("index.json")
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir().join(format!("{}.json", id))
    }

    fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("creating base dir {}", self.base_dir.display()))?;
        std::fs::create_dir_all(self.sessions_dir())
            .with_context(|| format!("creating sessions dir {}", self.sessions_dir().display()))?;
        Ok(())
    }

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

    fn save_index_locked(&mut self) -> Result<()> {
        let path = self.index_path();
        let json = serde_json::to_string_pretty(&self.index).context("serializing session index")?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .with_context(|| format!("writing index tmp file {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("renaming index tmp file to {}", path.display()))?;
        Ok(())
    }

    pub fn save_index(&mut self) -> Result<()> {
        self.lock.acquire(5000)?;
        let result = self.save_index_locked();
        self.lock.release()?;
        result
    }

    pub fn start_new(&mut self, project_path: Option<&Path>) -> Result<&Session> {
        let session = Session::new(project_path);
        self.current_session = Some(session);
        self.save_current()?;
        self.current_session.as_ref()
            .ok_or_else(|| anyhow::anyhow!("session not found after creation"))
    }

    pub fn continue_last(&mut self) -> Result<Option<&Session>> {
        let last_id = self.index.last_session().map(|m| m.id.clone());
        if let Some(id) = last_id {
            self.load_session(&id)?;
            Ok(self.current_session.as_ref())
        } else {
            Ok(None)
        }
    }

    pub fn resume(&mut self, query: &str) -> Result<Option<&Session>> {
        let session_id = self.index.find(query).map(|m| m.id.clone());
        if let Some(id) = session_id {
            self.load_session(&id)?;
            Ok(self.current_session.as_ref())
        } else {
            Ok(None)
        }
    }

    fn load_session(&mut self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if !path.exists() {
            anyhow::bail!("session file {} not found", path.display());
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading session file {}", path.display()))?;
        let mut session: Session = serde_json::from_str(&data)
            .with_context(|| format!("parsing session file {}", path.display()))?;

        session.migrate_legacy();

        if session.metadata.name.is_none()
            && let Some(index_meta) = self.index.find(id)
        {
            session.metadata.name = index_meta.name.clone();
        }

        self.current_session = Some(session);
        Ok(())
    }

    pub fn save_current(&mut self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            let session_clone = session.clone();

            self.lock.acquire(5000)?;

            self.index.upsert(session_clone.metadata.clone());
            self.save_index_locked()?;

            let path = self.session_path(&session_clone.metadata.id);
            let json = serde_json::to_string(&session_clone).context("serializing session")?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, json)
                .with_context(|| format!("writing session tmp file {}", tmp.display()))?;
            std::fs::rename(&tmp, &path)
                .with_context(|| format!("renaming session tmp file to {}", path.display()))?;

            self.lock.release()?;
        }
        Ok(())
    }

    pub fn update_stats(&mut self, last_input_tokens: u32, total_output_tokens: u64) {
        if let Some(ref mut session) = self.current_session {
            session.update_stats(last_input_tokens, total_output_tokens);
        }
    }

    pub fn record_compression(&mut self, entry: CompressionHistoryEntry) {
        if let Some(ref mut session) = self.current_session {
            session.metadata.add_compression_entry(entry);
        }
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        if let Some(ref mut session) = self.current_session {
            if session.metadata.name.is_none()
                && !messages.is_empty()
                && let Some(name) = Self::generate_name_from_messages(&messages)
            {
                session.metadata.name = Some(name);
            }

            session.full_messages = messages.clone();
            session.message_summaries = messages
                .iter()
                .enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect();
            session.metadata.message_count = session.full_messages.len();
            session.metadata.updated_at = Utc::now();
        }
    }

    pub fn set_compressed_messages(&mut self, compressed: Vec<Message>) {
        if let Some(ref mut session) = self.current_session {
            for summary in &mut session.message_summaries {
                summary.is_compressed = true;
            }

            for compressed_msg in &compressed {
                for (idx, full_msg) in session.full_messages.iter().enumerate() {
                    if session.message_summaries.get(idx).is_some() {
                        let same_role = compressed_msg.role == full_msg.role;
                        if same_role
                            && let Some(summary) = session.message_summaries.get_mut(idx) {
                                summary.is_compressed = false;
                            }
                    }
                }
            }

            session.compressed_messages = compressed;
        }
    }

    fn generate_name_from_messages(messages: &[Message]) -> Option<String> {
        let user_messages: Vec<&Message> = messages.iter().filter(|m| m.role == Role::User).collect();

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

            if cleaned.len() < 5 || is_generic_message(cleaned) {
                continue;
            }

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

    pub fn api_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.api_messages())
    }

    pub fn display_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.display_messages())
    }

    pub fn messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.api_messages())
    }

    pub fn messages_mut(&mut self) -> Option<&mut Vec<Message>> {
        self.current_session.as_mut().map(|s| &mut s.full_messages)
    }

    pub fn full_messages(&self) -> Option<&[Message]> {
        self.current_session.as_ref().map(|s| s.display_messages())
    }

    pub fn current_id(&self) -> Option<&str> {
        self.current_session.as_ref().map(|s| s.metadata.id.as_str())
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current_session.as_ref().and_then(|s| s.name())
    }

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

    pub fn clear_current(&mut self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            self.lock.acquire(5000)?;

            let path = self.session_path(&session.metadata.id);
            let _ = std::fs::remove_file(&path);
            self.index.remove(&session.metadata.id);
            self.save_index_locked()?;

            self.lock.release()?;
        }
        self.current_session = None;
        Ok(())
    }

    pub fn list_sessions(&self) -> &[SessionMetadata] {
        &self.index.sessions
    }

    pub fn cleanup_old_sessions(&mut self, max_age_days: u64) -> Result<usize> {
        let now = chrono::Utc::now();
        let threshold = chrono::Duration::days(max_age_days as i64);

        let mut to_remove: Vec<String> = Vec::new();

        for session in &self.index.sessions {
            let age = now - session.updated_at;
            if age > threshold {
                to_remove.push(session.id.clone());
            }
        }

        let removed_count = to_remove.len();

        if removed_count > 0 {
            self.lock.acquire(5000)?;

            for id in &to_remove {
                let path = self.session_path(id);
                let _ = std::fs::remove_file(&path);
                self.index.remove(id);
            }

            self.save_index_locked()?;
            self.lock.release()?;
        }

        Ok(removed_count)
    }

    pub fn prune_sessions(&mut self, max_sessions: usize) -> Result<usize> {
        if self.index.sessions.len() <= max_sessions {
            return Ok(0);
        }

        let to_remove = self.index.sessions.len() - max_sessions;
        let mut ids_to_remove: Vec<String> = Vec::new();

        for session in self.index.sessions.iter().skip(max_sessions) {
            ids_to_remove.push(session.id.clone());
        }

        self.lock.acquire(5000)?;

        for id in &ids_to_remove {
            let path = self.session_path(id);
            let _ = std::fs::remove_file(&path);
            self.index.remove(id);
        }

        self.save_index_locked()?;
        self.lock.release()?;

        Ok(to_remove)
    }

    pub fn session_count(&self) -> usize {
        self.index.sessions.len()
    }

    pub fn has_current(&self) -> bool {
        self.current_session.is_some()
    }

    pub fn current_metadata(&self) -> Option<&SessionMetadata> {
        self.current_session.as_ref().map(|s| &s.metadata)
    }

    pub fn history_path(&self) -> PathBuf {
        self.base_dir.join("history.txt")
    }
}