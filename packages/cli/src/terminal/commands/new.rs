//! /new command handler

use matrixcode_core::AgentEvent;

/// Handle /new command
pub async fn handle_new(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    session_mgr: &mut Option<matrixcode_core::SessionManager>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    if let Some(mgr) = session_mgr {
        let pp = std::env::current_dir().ok();
        mgr.start_new(pp.as_deref()).ok();
        agent.clear_history();
        let _ = event_tx.send(AgentEvent::session_ended()).await;
        let _ = event_tx.send(AgentEvent::progress("✓ New session created", None)).await;
    }
}