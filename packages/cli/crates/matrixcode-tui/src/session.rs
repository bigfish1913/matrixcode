//! Session Store
//!
//! Manages session persistence to JSON files.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Session data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Session ID
    pub id: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Messages in the session
    pub messages: Vec<SessionMessage>,
    /// Input history
    pub input_history: Vec<String>,
    /// Current model
    pub model: String,
    /// Project path
    pub project_path: String,
}

/// Session message for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Message role
    pub role: String,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Session store for persistence
pub struct SessionStore {
    /// Base path for session files
    base_path: PathBuf,
}

impl SessionStore {
    /// Create new session store
    pub fn new() -> Self {
        let base_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".matrixcode")
            .join("sessions");

        // Create directory if it doesn't exist
        if !base_path.exists() {
            fs::create_dir_all(&base_path).ok();
        }

        Self { base_path }
    }

    /// Get session file path
    fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("session-{}.json", id))
    }

    /// Get latest session file path
    fn latest_path(&self) -> PathBuf {
        self.base_path.join("latest.json")
    }

    /// Save session to file
    pub fn save(&self, session: &SessionData) -> Result<()> {
        let path = self.session_path(&session.id);
        let json = serde_json::to_string(session)?;
        fs::write(&path, json)?;

        // Update latest.json
        let latest = self.latest_path();
        fs::write(latest, serde_json::to_string(session)?)?;

        Ok(())
    }

    /// Load latest session
    pub fn load_latest(&self) -> Result<Option<SessionData>> {
        let path = self.latest_path();
        if path.exists() {
            let json = fs::read_to_string(path)?;
            let session: SessionData = serde_json::from_str(&json)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    /// Load specific session by ID
    pub fn load(&self, id: &str) -> Result<Option<SessionData>> {
        let path = self.session_path(id);
        if path.exists() {
            let json = fs::read_to_string(path)?;
            let session: SessionData = serde_json::from_str(&json)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    /// List all sessions
    pub fn list(&self) -> Result<Vec<SessionData>> {
        let mut sessions = Vec::new();

        if !self.base_path.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.base_path)? {
            let path = entry?.path();
            if path.extension().map(|e| e == "json").unwrap_or(false)
                && path.file_name().map(|n| n != "latest.json").unwrap_or(true)
            {
                if let Ok(json) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&json) {
                        sessions.push(session);
                    }
                }
            }
        }

        // Sort by updated_at descending
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }

    /// Delete session by ID
    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Create new session
    pub fn create(&self, project_path: Option<&str>) -> SessionData {
        let id = uuid::Uuid::new_v4().to_string();
        SessionData {
            id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: Vec::new(),
            input_history: Vec::new(),
            model: "claude-sonnet-4.6".to_string(),
            project_path: project_path.unwrap_or("").to_string(),
        }
    }

    /// Generate new session ID
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let store = SessionStore::new();
        let session = store.create(Some("/test/path"));

        assert!(!session.id.is_empty());
        assert_eq!(session.model, "claude-sonnet-4.6");
    }

    #[test]
    fn test_generate_id() {
        let id1 = SessionStore::generate_id();
        let id2 = SessionStore::generate_id();
        assert_ne!(id1, id2);
    }
}