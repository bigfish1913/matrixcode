//! Backend command execution context
//!
//! Provides access to all shared dependencies needed by backend commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use crate::{
    AgentEvent, Agent, Config, SessionManager,
    cancel::CancellationToken,
    memory::MemoryStorage,
    providers::Provider,
    skills::Skill,
};

/// Backend command execution context
///
/// Provides access to all shared dependencies needed by commands.
pub struct BackendContext<'a> {
    /// The full command message (e.g., "/workflow run test")
    pub message: &'a str,
    
    /// Event sender for progress messages
    pub event_tx: &'a Sender<AgentEvent>,
    
    /// Current project path
    pub project_path: Option<&'a PathBuf>,
    
    /// Available skills
    pub skills: &'a [Skill],
    
    /// Configuration
    pub config: &'a Config,
    
    /// Current model name
    pub model: &'a str,
    
    /// Session manager for /save, /load, /sessions
    pub session_mgr: &'a mut Option<SessionManager>,
    
    /// Memory storage for /memory commands
    pub memory_storage: &'a mut Option<MemoryStorage>,
    
    /// Agent instance for /compact, /tools
    pub agent: &'a mut Agent,
    
    /// Provider for AI calls
    pub provider: &'a dyn Provider,
    
    /// CodeGraph watcher handle (for /init)
    pub watcher_handle: Option<&'a Arc<Mutex<Option<JoinHandle<()>>>>>,
    
    /// Cancel token (for /init)
    pub cancel_token: Option<&'a CancellationToken>,
}