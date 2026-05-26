//! Session data and file locking

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::metadata::{SessionMetadata, MessageSummary};
use crate::providers::Message;

/// Full session data including messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub metadata: SessionMetadata,
    /// Full message history for display (TUI shows this).
    #[serde(default)]
    pub full_messages: Vec<Message>,
    /// Compressed messages for API requests (Agent uses this).
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
            message_summaries: messages
                .iter()
                .enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect(),
            messages,
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

    /// Get the session name (user-defined or fallback).
    pub fn name(&self) -> Option<&str> {
        self.metadata.name.as_deref()
    }

    /// Migrate legacy messages field to full_messages.
    pub fn migrate_legacy(&mut self) {
        if !self.messages.is_empty() && self.full_messages.is_empty() {
            log::info!(
                "Migrating legacy session: {} messages -> full_messages",
                self.messages.len()
            );
            self.full_messages = self.messages.clone();
            self.message_summaries = self
                .messages
                .iter()
                .enumerate()
                .map(|(i, m)| MessageSummary::from_message(m, i))
                .collect();
            self.messages.clear();
            log::info!(
                "Migration complete: full_messages={}, summaries={}",
                self.full_messages.len(),
                self.message_summaries.len()
            );
        }
    }
}

/// File lock for preventing concurrent access to session storage.
pub struct SessionFileLock {
    lock_path: PathBuf,
    locked: bool,
}

impl SessionFileLock {
    pub fn new(base_dir: &Path) -> Self {
        Self {
            lock_path: base_dir.join("sessions.lock"),
            locked: false,
        }
    }

    /// Acquire the lock (blocking with timeout).
    pub fn acquire(&mut self, timeout_ms: u64) -> anyhow::Result<()> {
        if self.locked {
            return Ok(());
        }

        let start = std::time::Instant::now();

        while start.elapsed().as_millis() < timeout_ms as u128 {
            match std::fs::File::create_new(&self.lock_path) {
                Ok(_) => {
                    let lock_info = format!("{}:{}", std::process::id(), Utc::now().to_rfc3339());
                    std::fs::write(&self.lock_path, lock_info)?;
                    self.locked = true;
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if self.is_stale_lock()? {
                        self.remove_stale_lock()?;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        anyhow::bail!("Failed to acquire session lock after {}ms timeout", timeout_ms)
    }

    fn is_stale_lock(&self) -> anyhow::Result<bool> {
        if !self.lock_path.exists() {
            return Ok(false);
        }

        if let Ok(content) = std::fs::read_to_string(&self.lock_path)
            && let Some(pid_str) = content.split(':').next()
            && let Ok(pid) = pid_str.parse::<u32>()
            && !self.is_process_running(pid)
        {
            return Ok(true);
        }

        let metadata = std::fs::metadata(&self.lock_path)?;
        let modified = metadata.modified()?;
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or(std::time::Duration::ZERO);

        Ok(age > std::time::Duration::from_secs(60))
    }

    fn is_process_running(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            std::path::Path::new(&format!("/proc/{}", pid)).exists()
        }
        #[cfg(windows)]
        {
            use std::process::Command;
            let output = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output();

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    stdout.contains(&pid.to_string()) && !stdout.contains("No tasks")
                }
                Err(_) => true,
            }
        }
    }

    fn remove_stale_lock(&self) -> anyhow::Result<()> {
        if self.lock_path.exists() {
            std::fs::remove_file(&self.lock_path)?;
        }
        Ok(())
    }

    pub fn release(&mut self) -> anyhow::Result<()> {
        if self.locked {
            std::fs::remove_file(&self.lock_path)?;
            self.locked = false;
        }
        Ok(())
    }
}

impl Drop for SessionFileLock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}