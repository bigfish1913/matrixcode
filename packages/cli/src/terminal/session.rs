//! Session management for terminal mode
//!
//! Handles session creation, restoration, listing, and cleanup.

use anyhow::Result;
use matrixcode_core::SessionManager;
use crate::constants::{DEFAULT_MAX_TOKENS, SESSION_CLEANUP_DAYS, DISPLAY_SESSIONS_LIMIT};
use crate::types::Cli;
use super::setup::run_terminal_mode;

/// Interactive session resume - list sessions and let user select
pub fn interactive_resume() -> Result<()> {
    use std::io::{self, Write};

    let _ = matrixcode_tui::crossterm::terminal::disable_raw_mode();

    let mgr = SessionManager::new()?;
    let sessions = mgr.list_sessions();

    if sessions.is_empty() {
        println!("No sessions found.");
        println!("\nTip: Use 'matrixcode' to start a new session.");
        return Ok(());
    }

    println!("📚 Sessions:\n");
    for (i, session) in sessions.iter().enumerate() {
        let is_current = mgr.has_current() && mgr.current_id() == Some(session.id.as_str());
        println!("  {}. {}", i + 1, session.format_line(is_current));
    }

    println!("\nSelect session to resume (1-{}), or 'q' to quit:", sessions.len());
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selection = input.trim().to_string();

    if matches!(selection.as_str(), "q" | "quit" | "exit" | "") {
        println!("Cancelled.");
        return Ok(());
    }

    // Try to parse as number
    if let Ok(num) = selection.parse::<usize>()
        && num > 0
        && num <= sessions.len()
    {
        let session = &sessions[num - 1];
        println!("\n✓ Resuming session: {}", session.short_id());
        println!("  Project: {}", session.project_path.as_deref().unwrap_or("unknown"));
        println!("  Messages: {}", session.message_count);
        println!("\nStarting matrixcode with resumed session...\n");

        let cli = Cli {
            mode: "terminal".to_string(),
            continue_session: false,
            resume: false,
            resume_id: Some(session.id.clone()),
            list_sessions: false,
            skills_dir: None,
            think: Some(true),
            max_tokens: DEFAULT_MAX_TOKENS,
            mcp: Vec::new(),
            no_mcp: false,
            command: None,
        };
        return run_terminal_mode(cli);
    }

    // Try to match by short_id or full id
    for session in sessions.iter() {
        if session.short_id() == selection || session.id == selection || session.id.starts_with(&selection) {
            println!("\n✓ Resuming session: {}", session.short_id());
            println!("  Project: {}", session.project_path.as_deref().unwrap_or("unknown"));
            println!("  Messages: {}", session.message_count);
            println!("\nStarting matrixcode with resumed session...\n");

            let cli = Cli {
                mode: "terminal".to_string(),
                continue_session: false,
                resume: false,
                resume_id: Some(session.id.clone()),
                list_sessions: false,
                skills_dir: None,
                think: Some(true),
                max_tokens: DEFAULT_MAX_TOKENS,
                mcp: Vec::new(),
                no_mcp: false,
                command: None,
            };
            return run_terminal_mode(cli);
        }
    }

    println!("Unknown session: {}", selection);
    Ok(())
}

/// List sessions to stdout
pub fn list_sessions() {
    let mgr = SessionManager::new().ok();
    if let Some(mgr) = mgr {
        let sessions = mgr.list_sessions();
        if sessions.is_empty() {
            println!("No sessions found.");
            println!("\nTip: Use 'matrixcode' to start a new session.");
        } else {
            println!("Sessions:\n");
            for (i, session) in sessions.iter().enumerate() {
                let status = if mgr.has_current() && mgr.current_id() == Some(session.id.as_str()) {
                    " [current]"
                } else {
                    ""
                };
                let project = session.project_path.as_deref().unwrap_or("unknown");
                println!("  {}. {} ({}){}", i + 1, session.short_id(), project, status);
            }
            println!("\nTotal: {} sessions", sessions.len());
            println!("\nResume: matrixcode --resume <id>");
        }
    } else {
        println!("No session manager available.");
        println!("Sessions directory: ~/.matrix/sessions/");
    }
}

/// Handle /save command
pub async fn handle_save(
    event_tx: &tokio::sync::mpsc::Sender<matrixcode_core::AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
    messages: &[matrixcode_core::providers::Message],
) {
    let parts: Vec<&str> = msg.split_whitespace().collect();
    let name = parts.get(1).copied();

    if let Some(mgr) = session_mgr {
        mgr.set_messages(messages.to_vec());
        if let Some(n) = name
            && let Err(e) = mgr.rename_current(n) {
                let _ = event_tx.send(matrixcode_core::AgentEvent::error(
                    format!("Failed to rename: {}", e), None, None
                )).await;
            }
        if let Err(e) = mgr.save_current() {
            let _ = event_tx.send(matrixcode_core::AgentEvent::error(
                format!("Failed to save: {}", e), None, None
            )).await;
        } else {
            let _ = event_tx.send(matrixcode_core::AgentEvent::progress("✓ Session saved", None)).await;
        }
    } else {
        let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
            "❌ Session manager not available", None
        )).await;
    }
}

/// Handle /sessions and /resume commands
pub async fn handle_sessions(
    event_tx: &tokio::sync::mpsc::Sender<matrixcode_core::AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
) {
    let subcmd = if msg.starts_with("/sessions ") {
        msg.strip_prefix("/sessions ").unwrap_or("")
    } else {
        ""
    };

    if let Some(mgr) = session_mgr {
        if subcmd == "cleanup" {
            let removed = mgr.cleanup_old_sessions(SESSION_CLEANUP_DAYS).unwrap_or(0);
            let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
                format!("✓ Removed {} old sessions", removed), None
            )).await;
        } else if subcmd == "stats" {
            let sessions = mgr.list_sessions();
            let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
                format!("📊 {} sessions total", sessions.len()), None
            )).await;
        } else {
            let sessions = mgr.list_sessions();
            if sessions.is_empty() {
                let _ = event_tx.send(matrixcode_core::AgentEvent::progress("No saved sessions", None)).await;
            } else {
                let mut info = format!("📚 Sessions ({}):\n", sessions.len());
                for session in sessions.iter().take(DISPLAY_SESSIONS_LIMIT) {
                    info.push_str(&format!("• {} - {} msgs\n", session.short_id(), session.message_count));
                }
                let _ = event_tx.send(matrixcode_core::AgentEvent::progress(info, None)).await;
            }
        }
    } else {
        let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
            "❌ Session manager not available", None
        )).await;
    }
}

/// Handle /load command
pub async fn handle_load(
    event_tx: &tokio::sync::mpsc::Sender<matrixcode_core::AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    let session_id = msg.strip_prefix("/load ").unwrap_or("");

    if let Some(mgr) = session_mgr {
        if mgr.resume(session_id).is_ok() {
            if let Some(msgs) = mgr.messages() {
                let messages = msgs.to_vec();
                agent.set_messages(messages.clone());
                let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
                    format!("✓ Session '{}' loaded ({} messages)", session_id, messages.len()),
                    None,
                )).await;
            }
        } else {
            let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
                format!("❌ Session '{}' not found", session_id), None
            )).await;
        }
    } else {
        let _ = event_tx.send(matrixcode_core::AgentEvent::progress(
            "❌ Session manager not available", None
        )).await;
    }
}

/// Save session after agent turn
pub async fn save_after_turn(
    event_tx: &tokio::sync::mpsc::Sender<matrixcode_core::AgentEvent>,
    session_mgr: &mut Option<SessionManager>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    if let Some(mgr) = session_mgr {
        let (input_tokens, output_tokens) = agent.get_token_counts();
        let messages = agent.get_messages();
        mgr.set_messages(messages.to_vec());
        mgr.set_compressed_messages(messages.to_vec());
        mgr.update_stats(input_tokens as u32, output_tokens);
        if let Err(e) = mgr.save_current() {
            let _ = event_tx.send(matrixcode_core::AgentEvent::error(
                format!("Session save failed: {}", e), None, None
            )).await;
        }
        matrixcode_core::debug::debug_log().session_save(messages.len(), output_tokens);
    }
}