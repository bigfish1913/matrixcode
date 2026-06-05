//! /init command handler

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use matrixcode_core::{
    AgentEvent, cancel::CancellationToken, providers::Provider,
    tools::codegraph::{get_codegraph_path, should_inject_codegraph_tools, CodeGraphManager},
};
use crate::commands::{handle_init_command, InitCommandResult};
use super::super::watcher::ensure_watcher_running;


/// Handle /init command
/// Returns true if CodeGraph was initialized (indicating tools need refresh)
pub async fn handle_init(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    project_path: &Option<PathBuf>,
    provider: &dyn Provider,
    watcher_handle: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    cancel_token: &CancellationToken,
) -> bool {
    let result = handle_init_command(msg, project_path.as_deref());
    match result {
        InitCommandResult::Message(msg) => {
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::Progress,
                matrixcode_core::EventData::Progress {
                    message: msg,
                    percentage: None,
                },
            )).await;
            false
        }
        InitCommandResult::GenerateOverview => {
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::Progress,
                matrixcode_core::EventData::Progress {
                    message: "🔄 Generating project overview...".into(),
                    percentage: Some(10),
                },
            )).await;

            if let Some(path) = project_path {
                // Step 1: Generate project overview
                let overview_result = matrixcode_core::overview::ProjectOverview::generate_with_ai(path.as_path(), provider).await;

                match overview_result {
                    Ok(overview) => {
                        let _ = event_tx.send(AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: format!("✓ Project overview generated: {}", overview.path.display()),
                                percentage: Some(50),
                            },
                        )).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::error(
                            format!("Failed to generate overview: {}", e),
                            Some("overview_error".into()),
                            None,
                        )).await;
                        return false;
                    }
                }

                // Step 2: Initialize CodeGraph if CLI is installed and db doesn't exist
                let cli_installed = get_codegraph_path().is_some();
                let db_exists = should_inject_codegraph_tools(path);

                if cli_installed && !db_exists {
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "🔄 Generating CodeGraph index...".into(),
                            percentage: Some(60),
                        },
                    )).await;

                    let manager = CodeGraphManager::new(path);
                    match manager.init().await {
                        Ok(_) => {
                            // Sync after init
                            if let Err(e) = manager.sync().await {
                                log::warn!("CodeGraph sync failed: {}", e);
                            }

                            // Step 3: Check daemon status and start watcher if no conflict
                            ensure_watcher_running(path, cancel_token.clone(), watcher_handle, event_tx.clone());

                            let _ = event_tx.send(AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: "✓ CodeGraph index generated (code analysis tools now available)".into(),
                                    percentage: Some(100),
                                },
                            )).await;

                            // Return true to refresh tools (state changed)
                            true
                        }
                        Err(e) => {
                            let _ = event_tx.send(AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: format!("⚠️ CodeGraph generation skipped: {}", e),
                                    percentage: Some(100),
                                },
                            )).await;
                            false
                        }
                    }
                } else if !cli_installed {
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "⚠️ CodeGraph CLI not installed. Run 'codegraph install' to enable code analysis tools.".into(),
                            percentage: Some(100),
                        },
                    )).await;
                    false
                } else {
                    // db already exists
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "✓ CodeGraph index already exists".into(),
                            percentage: Some(100),
                        },
                    )).await;
                    false
                }
            } else {
                let _ = event_tx.send(AgentEvent::error(
                    String::from("No project path set. Cannot generate overview."),
                    Some("no_project".into()),
                    None,
                )).await;
                false
            }
        }
    }
}