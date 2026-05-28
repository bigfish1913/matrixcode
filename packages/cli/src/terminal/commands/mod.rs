//! Backend command handlers for terminal mode
//!
//! Uses strategy pattern with metadata for extensible command handling.
//! Core commands are handled by matrixcode_core::command module.

mod init;
mod new;
mod mode;
pub mod skill_activation;

pub use init::handle_init;
pub use new::handle_new;
pub use mode::handle_mode;

use matrixcode_core::AgentEvent;
use matrixcode_core::command::{get_registry, BackendContext};

/// Command execution context (shared dependencies)
pub struct CommandContext<'a> {
    pub event_tx: &'a tokio::sync::mpsc::Sender<AgentEvent>,
    pub project_path: &'a Option<std::path::PathBuf>,
    pub skills: &'a [matrixcode_core::skills::Skill],
    pub config: &'a matrixcode_core::Config,
    pub model: &'a str,
    pub session_mgr: &'a mut Option<matrixcode_core::SessionManager>,
    pub memory_storage: &'a mut Option<matrixcode_core::memory::MemoryStorage>,
}

/// Check if a message is a backend command
pub fn is_backend_command(msg: &str, skills: &[matrixcode_core::skills::Skill]) -> bool {
    // Known backend commands
    let known_commands = [
        "/init", "/skills", "/workflow", "/compact", "/compress",
        "/memory", "/overview", "/save", "/sessions", "/resume",
        "/load", "/config", "/tools", "/system", "/new", "/mode:",
    ];
    
    for cmd in known_commands {
        if msg.starts_with(cmd) || msg == cmd {
            return true;
        }
    }
    
    // Skill activation (matches skill name but not other commands)
    if msg.starts_with('/') && !msg.starts_with("/skills") {
        let skill_name = msg.trim_start_matches('/');
        return skills.iter().any(|s| s.name == skill_name);
    }
    
    false
}

/// Handle backend commands
pub async fn handle_command(
    msg: &str,
    ctx: &mut CommandContext<'_>,
    agent: &mut matrixcode_core::agent::Agent,
    watcher_handle: &std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    cancel_token: &matrixcode_core::cancel::CancellationToken,
    provider: &dyn matrixcode_core::providers::Provider,
) -> bool {
    use matrixcode_core::AgentEvent;
    
    // Handle /init first (special Git path normalization + watcher dependency)
    if msg.starts_with("/init") || (msg.contains("/init") && 
        (msg.contains("Program Files/Git") || msg.contains("Git/init"))) {
        let normalized_msg = if msg.contains("Program Files/Git") || msg.contains("Git/init") {
            "/init".to_string()
        } else {
            msg.to_string()
        };
        return handle_init(
            ctx.event_tx,
            &normalized_msg,
            ctx.project_path,
            provider,
            watcher_handle,
            cancel_token,
        ).await;
    }
    
    // Handle skill activation (check if skill name matches)
    if msg.starts_with('/') && !msg.starts_with("/skills") {
        let skill_name = msg.trim_start_matches('/');
        if let Some(skill) = ctx.skills.iter().find(|s| s.name == skill_name) {
            let _ = ctx.event_tx.send(AgentEvent::progress(
                format!("🎯 Activating skill: {}", skill.name),
                None,
            )).await;
            return false;
        }
    }
    
    // Try core command registry first
    let registry = get_registry();
    let mut backend_ctx = BackendContext {
        message: msg,
        event_tx: ctx.event_tx,
        project_path: ctx.project_path.as_ref(),
        skills: ctx.skills,
        config: ctx.config,
        model: ctx.model,
        session_mgr: ctx.session_mgr,
        memory_storage: ctx.memory_storage,
        agent: agent,
        provider: provider,
        watcher_handle: None,
        cancel_token: None,
    };
    
    if let Some(result) = registry.dispatch(msg, &mut backend_ctx).await {
        return result;
    }
    
    // Fallback: commands not in core registry
    
    // Handle /save
    if msg == "/save" || msg.starts_with("/save ") {
        super::session::handle_save(ctx.event_tx, msg, ctx.session_mgr, agent.get_messages()).await;
        return false;
    }
    
    // Handle /sessions
    if msg == "/sessions" || msg == "/resume" || msg.starts_with("/sessions ") {
        super::session::handle_sessions(ctx.event_tx, msg, ctx.session_mgr).await;
        return false;
    }
    
    // Handle /load
    if msg.starts_with("/load ") {
        super::session::handle_load(ctx.event_tx, msg, ctx.session_mgr, agent).await;
        return false;
    }
    
    // Handle /new
    if msg == "/new" {
        handle_new(ctx.event_tx, ctx.session_mgr, agent).await;
        return false;
    }
    
    // Handle /mode:
    if msg.starts_with("/mode:") {
        handle_mode(msg, agent);
        return false;
    }
    
    false
}