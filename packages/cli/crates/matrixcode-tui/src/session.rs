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

    /// Create session store with custom base path (for testing)
    #[cfg(test)]
    pub fn with_path(base_path: PathBuf) -> Self {
        if !base_path.exists() {
            fs::create_dir_all(&base_path).ok();
        }
        Self { base_path }
    }

    /// Get the base path (for testing)
    #[cfg(test)]
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Get session file path
    #[cfg(test)]
    pub fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("session-{}.json", id))
    }

    #[cfg(not(test))]
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

    // ===== SessionData Tests =====

    #[test]
    fn test_session_data_serialization() {
        let session = SessionData {
            id: "test-id-123".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: vec![SessionMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                timestamp: Utc::now(),
            }],
            input_history: vec!["cmd1".to_string(), "cmd2".to_string()],
            model: "claude-sonnet-4.6".to_string(),
            project_path: "/test/path".to_string(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: SessionData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, session.id);
        assert_eq!(parsed.model, session.model);
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.input_history.len(), 2);
    }

    #[test]
    fn test_session_message_serialization() {
        let msg = SessionMessage {
            role: "assistant".to_string(),
            content: "Response text".to_string(),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: SessionMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.role, msg.role);
        assert_eq!(parsed.content, msg.content);
    }

    // ===== SessionStore Tests with temp directory =====

    /// Helper to create a temp session store
    fn create_temp_store() -> (SessionStore, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = SessionStore::with_path(temp_dir.path().to_path_buf());
        (store, temp_dir)
    }

    #[test]
    fn test_create_session() {
        let store = SessionStore::new();
        let session = store.create(Some("/test/path"));

        assert!(!session.id.is_empty());
        assert_eq!(session.model, "claude-sonnet-4.6");
        assert!(session.messages.is_empty());
        assert!(session.input_history.is_empty());
    }

    #[test]
    fn test_create_session_no_project() {
        let store = SessionStore::new();
        let session = store.create(None);

        assert!(!session.id.is_empty());
        assert_eq!(session.project_path, "");
    }

    #[test]
    fn test_generate_id() {
        let id1 = SessionStore::generate_id();
        let id2 = SessionStore::generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_id_format() {
        let id = SessionStore::generate_id();
        // UUID v4 format: 8-4-4-4-12 hex characters
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }

    #[test]
    fn test_save_and_load_session() {
        let (store, _temp) = create_temp_store();
        let mut session = store.create(Some("/test/project"));
        session.messages.push(SessionMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
        });
        session.input_history.push("test command".to_string());

        // Save
        let result = store.save(&session);
        assert!(result.is_ok());

        // Load
        let loaded = store.load(&session.id).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.model, session.model);
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.input_history.len(), 1);
    }

    #[test]
    fn test_load_nonexistent_session() {
        let (store, _temp) = create_temp_store();
        let result = store.load("nonexistent-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_latest() {
        let (store, _temp) = create_temp_store();

        // No latest session initially
        let result = store.load_latest().unwrap();
        assert!(result.is_none());

        // Create and save a session
        let session = store.create(Some("/project1"));
        store.save(&session).unwrap();

        // Load latest
        let loaded = store.load_latest().unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, session.id);
    }

    #[test]
    fn test_load_latest_updates_on_save() {
        let (store, _temp) = create_temp_store();

        // Create and save first session
        let session1 = store.create(Some("/project1"));
        store.save(&session1).unwrap();

        // Load latest should return session1
        let latest = store.load_latest().unwrap().unwrap();
        assert_eq!(latest.id, session1.id);

        // Create and save second session
        let session2 = store.create(Some("/project2"));
        store.save(&session2).unwrap();

        // Load latest should now return session2
        let latest = store.load_latest().unwrap().unwrap();
        assert_eq!(latest.id, session2.id);
    }

    #[test]
    fn test_list_sessions_empty() {
        let (store, _temp) = create_temp_store();
        let sessions = store.list().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_sessions() {
        let (store, _temp) = create_temp_store();

        // Create multiple sessions
        let session1 = store.create(Some("/project1"));
        let session2 = store.create(Some("/project2"));
        let session3 = store.create(Some("/project3"));

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mut session2_updated = session2.clone();
        session2_updated.updated_at = Utc::now();

        store.save(&session1).unwrap();
        store.save(&session2_updated).unwrap();
        store.save(&session3).unwrap();

        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 3);

        // Should be sorted by updated_at descending
        // session2_updated has the latest timestamp
        assert_eq!(sessions[0].id, session2_updated.id);
    }

    #[test]
    fn test_list_sessions_excludes_latest_json() {
        let (store, _temp) = create_temp_store();

        // Create and save a session
        let session = store.create(Some("/project"));
        store.save(&session).unwrap();

        // List should only return 1 session (not counting latest.json)
        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_delete_session() {
        let (store, _temp) = create_temp_store();

        let session = store.create(Some("/project"));
        store.save(&session).unwrap();

        // Verify session exists
        let loaded = store.load(&session.id).unwrap();
        assert!(loaded.is_some());

        // Delete
        let result = store.delete(&session.id);
        assert!(result.is_ok());

        // Verify session is gone
        let loaded = store.load(&session.id).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_delete_nonexistent_session() {
        let (store, _temp) = create_temp_store();
        let result = store.delete("nonexistent-id");
        assert!(result.is_ok()); // Should not error
    }

    #[test]
    fn test_session_path_format() {
        let (store, _temp) = create_temp_store();
        let path = store.session_path("test-123");
        assert!(path.to_str().unwrap().contains("session-test-123.json"));
    }

    #[test]
    fn test_session_persistence_with_messages() {
        let (store, _temp) = create_temp_store();

        let mut session = store.create(Some("/test/project"));
        session.messages.push(SessionMessage {
            role: "user".to_string(),
            content: "What is the weather?".to_string(),
            timestamp: Utc::now(),
        });
        session.messages.push(SessionMessage {
            role: "assistant".to_string(),
            content: "I don't have access to real-time weather data.".to_string(),
            timestamp: Utc::now(),
        });
        session.input_history.push("weather question".to_string());
        session.input_history.push("another question".to_string());

        store.save(&session).unwrap();
        let loaded = store.load(&session.id).unwrap().unwrap();

        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].role, "user");
        assert_eq!(loaded.messages[1].role, "assistant");
        assert_eq!(loaded.input_history.len(), 2);
    }

    #[test]
    fn test_session_default_store() {
        let store = SessionStore::default();
        // Should create a store with home directory path
        assert!(store.base_path().to_str().unwrap().contains(".matrixcode"));
    }

    #[test]
    fn test_session_data_clone() {
        let session = SessionData {
            id: "test-id".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: vec![],
            input_history: vec!["cmd".to_string()],
            model: "claude-sonnet-4.6".to_string(),
            project_path: "/path".to_string(),
        };

        let cloned = session.clone();
        assert_eq!(cloned.id, session.id);
        assert_eq!(cloned.model, session.model);
    }

    #[test]
    fn test_session_message_clone() {
        let msg = SessionMessage {
            role: "user".to_string(),
            content: "test".to_string(),
            timestamp: Utc::now(),
        };

        let cloned = msg.clone();
        assert_eq!(cloned.role, msg.role);
        assert_eq!(cloned.content, msg.content);
    }
}