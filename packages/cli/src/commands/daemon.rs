//! Daemon mode for MatrixCode CLI
//!
//! Provides a persistent background process that can receive requests
//! via stdin and respond with JSON events.

use anyhow::Result;
use matrixcode_core::{
    AgentEvent, AgentBuilder, Config,
    create_provider_with_headers,
    tools::all_tools_with_box_provider,
    approval::ApproveMode,
    session::SessionManager,
};
use serde::Deserialize;
use std::io::{BufRead, Write};
use std::sync::Arc;

use crate::helpers::{
    resolve_provider, resolve_model_with_override, resolve_base_url,
    load_skills, build_quick_action_prompt, model_with_source,
};

/// Daemon request
#[derive(Deserialize)]
pub struct DaemonRequest {
    #[serde(rename = "type")]
    request_type: String,
    content: Option<String>,
    action: Option<String>,
    file: Option<String>,
    session_id: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
}

/// Run daemon mode - persistent background process
pub fn run_daemon_mode() -> Result<()> {
    eprintln!("MatrixCode Daemon started (listening on stdin)");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;

        if line.is_empty() {
            continue;
        }

        let request: DaemonRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let error_event = AgentEvent::error(
                    format!("Invalid request: {}", e),
                    Some("parse_error".to_string()),
                    None,
                );
                writeln!(stdout_lock, "{}", error_event.to_json()?)?;
                writeln!(stdout_lock, "---END---")?;
                stdout_lock.flush()?;
                continue;
            }
        };

        let events = handle_daemon_request(request)?;

        for event in events {
            writeln!(stdout_lock, "{}", event.to_json()?)?;
        }

        writeln!(stdout_lock, "---END---")?;
        stdout_lock.flush()?;
    }

    Ok(())
}

fn handle_daemon_request(request: DaemonRequest) -> Result<Vec<AgentEvent>> {
    let mut events = Vec::new();
    let config = Config::load();
    let skills = load_skills(&[]);

    events.push(AgentEvent::session_started());

    match request.request_type.as_str() {
        "chat" => {
            if let Some(content) = request.content {
                let api_key = config.api_key.clone()
                    .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                    .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

                let model = resolve_model_with_override(request.model.clone(), &config);
                let base_url = resolve_base_url(&config);
                let max_tokens = request.max_tokens.unwrap_or(4096);

                let rt = tokio::runtime::Runtime::new()?;
                let result = rt.block_on(async {
                    let provider = create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
                    )?;

                    let mut agent = AgentBuilder::new(provider.clone_box())
                        .model_name(model)
                        .max_tokens(max_tokens)
                        .tools(all_tools_with_box_provider(Arc::new(skills.clone()), provider.clone_box()))
                        .approve_mode(ApproveMode::Auto)
                        .build();

                    agent.run(content).await
                });

                match result {
                    Ok(_) => events.push(AgentEvent::text_delta("Chat completed".to_string())),
                    Err(e) => events.push(AgentEvent::error(format!("Chat failed: {}", e), None, None)),
                }
            } else {
                events.push(AgentEvent::error("No content provided for chat", None, None));
            }
        }

        "quick_action" => {
            if let Some(action) = request.action.clone() {
                let prompt = build_quick_action_prompt(&action, request.file.as_ref());

                let api_key = config.api_key.clone()
                    .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                    .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

                let model = resolve_model_with_override(request.model.clone(), &config);
                let base_url = resolve_base_url(&config);

                events.push(AgentEvent::tool_use_start("action_1", action.clone(), None));

                let rt = tokio::runtime::Runtime::new()?;
                let result = rt.block_on(async {
                    let provider = create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
                    )?;

                    let mut agent = AgentBuilder::new(provider.clone_box())
                        .model_name(model)
                        .max_tokens(4096)
                        .tools(all_tools_with_box_provider(Arc::new(skills.clone()), provider.clone_box()))
                        .approve_mode(ApproveMode::Auto)
                        .build();

                    agent.run(prompt).await
                });

                match result {
                    Ok(_) => events.push(AgentEvent::tool_result("action_1", "action", None, "Action completed", false)),
                    Err(e) => events.push(AgentEvent::tool_result("action_1", "action", None, format!("Error: {}", e), true)),
                }
            } else {
                events.push(AgentEvent::error("No action specified", None, None));
            }
        }

        "status" => {
            let status = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "mode": "daemon",
                "api_configured": config.is_api_configured(),
                "model": model_with_source(&config),
            });
            events.push(AgentEvent::progress(serde_json::to_string(&status)?, None));
        }

        "history" | "list_sessions" => {
            if let Ok(mgr) = SessionManager::new() {
                let sessions = mgr.list_sessions();
                let sessions_json: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| serde_json::json!({
                        "id": s.id,
                        "short_id": s.short_id(),
                        "project_path": s.project_path,
                        "created_at": s.created_at.to_rfc3339(),
                        "message_count": s.message_count,
                    }))
                    .collect();

                let data = serde_json::json!({
                    "type": "history",
                    "sessions": sessions_json,
                    "total": sessions.len()
                });
                events.push(AgentEvent::progress(serde_json::to_string(&data)?, None));
            } else {
                events.push(AgentEvent::error("Session manager not available", None, None));
            }
        }

        "new_session" => {
            if let Ok(mut mgr) = SessionManager::new() {
                let project_path = std::env::current_dir().ok();
                match mgr.start_new(project_path.as_deref()) {
                    Ok(_) => {
                        let data = serde_json::json!({
                            "success": true,
                            "session_id": mgr.current_id(),
                            "message": "New session created"
                        });
                        events.push(AgentEvent::progress(serde_json::to_string(&data)?, None));
                    }
                    Err(e) => {
                        events.push(AgentEvent::error(format!("Failed to create session: {}", e), None, None));
                    }
                }
            } else {
                events.push(AgentEvent::error("Session manager not available", None, None));
            }
        }

        "load_session" => {
            if let Some(session_id) = request.session_id.clone() {
                if let Ok(mut mgr) = SessionManager::new() {
                    match mgr.resume(&session_id) {
                        Ok(Some(session)) => {
                            let data = serde_json::json!({
                                "success": true,
                                "session_id": session.metadata.id,
                                "message_count": session.messages.len(),
                                "message": "Session loaded"
                            });
                            events.push(AgentEvent::progress(serde_json::to_string(&data)?, None));
                        }
                        Ok(None) => {
                            events.push(AgentEvent::error(format!("Session '{}' not found", session_id), None, None));
                        }
                        Err(e) => {
                            events.push(AgentEvent::error(format!("Failed to load session: {}", e), None, None));
                        }
                    }
                } else {
                    events.push(AgentEvent::error("Session manager not available", None, None));
                }
            } else {
                events.push(AgentEvent::error("No session_id provided", None, None));
            }
        }

        "ping" => {
            events.push(AgentEvent::text_delta("pong".to_string()));
        }

        _ => {
            events.push(AgentEvent::error(
                format!("Unknown request type: {}", request.request_type),
                Some("unknown_type".to_string()),
                None,
            ));
        }
    }

    events.push(AgentEvent::session_ended());
    Ok(events)
}