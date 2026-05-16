// ============================================================================
// IPC Daemon for VSCode Extension Integration
// ============================================================================
//
// This module implements the daemon mode for VSCode extension integration.
// When running with --daemon --json flags:
// - Reads JSON requests from stdin
// - Processes requests using the Agent
// - Streams JSON events to stdout (JSON Lines format)

use anyhow::Result;
use std::io::{BufRead, BufReader, Write, stdout, stdin};
use std::sync::Arc;

use crate::agent::Agent;
use crate::protocol::{ClientRequest, StreamEvent, RequestContext, QuickActionType};
use crate::session::SessionManager;
use crate::cancel::CancellationToken;

/// Run the daemon loop, processing JSON requests from stdin
pub async fn run_daemon(
    mut agent: Agent,
    mut session_manager: SessionManager,
    project_root: Option<std::path::PathBuf>,
) -> Result<()> {
    // Send session started event
    let session_id = session_manager.current_metadata()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "new".to_string());
    
    print_event(StreamEvent::session_started(session_id, None));
    
    // Create cancellation token
    let cancel_token = Arc::new(CancellationToken::new());
    
    // Set up buffered reader for stdin
    let reader = BufReader::new(stdin());
    
    // Process requests line by line
    for line in reader.lines() {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            print_event(StreamEvent::Log {
                level: "info".to_string(),
                message: "Daemon cancelled, shutting down".to_string(),
            });
            break;
        }
        
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                // stdin closed or error
                log::debug!("stdin error: {}", e);
                break;
            }
        };
        
        if line.trim().is_empty() {
            continue;
        }
        
        // Parse request
        let request: ClientRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                print_event(StreamEvent::error(format!("Invalid request: {}", e)));
                continue;
            }
        };
        
        // Process request
        match request {
            ClientRequest::Chat { content, context } => {
                handle_chat(&mut agent, &mut session_manager, content, context).await;
            }
            
            ClientRequest::QuickAction { action, content, context, instructions } => {
                handle_quick_action(&mut agent, &mut session_manager, action, content, context, instructions).await;
            }
            
            ClientRequest::NewSession => {
                handle_new_session(&mut agent, &mut session_manager, project_root.as_deref());
            }
            
            ClientRequest::Status => {
                handle_status(&agent, &session_manager);
            }
            
            ClientRequest::Memory { operation } => {
                handle_memory(operation);
            }
            
            ClientRequest::LoadSession { session_id } => {
                handle_load_session(&mut agent, &mut session_manager, session_id, project_root.as_deref());
            }
            
            ClientRequest::ListSessions => {
                handle_list_sessions(&session_manager);
            }
        }
    }
    
    Ok(())
}

/// Handle a chat request
async fn handle_chat(
    agent: &mut Agent,
    session_manager: &mut SessionManager,
    content: String,
    context: Option<RequestContext>,
) {
    // If context provided, we could enhance the message with file info
    let message = if let Some(ctx) = context {
        enhance_message_with_context(content, &ctx)
    } else {
        content
    };
    
    // Chat with streaming JSON output
    let result = agent.chat_stream_json(&message).await;
    
    match result {
        Ok(usage) => {
            // Save session state after successful chat
            let stats = agent.token_stats();
            session_manager.set_messages(agent.messages().to_vec());
            session_manager.update_stats(stats.last_input_tokens, stats.total_output_tokens);
            if let Err(e) = session_manager.save_current() {
                print_event(StreamEvent::error(format!("Failed to save session: {}", e)));
            }
            print_event(StreamEvent::done(Some(usage)));
        }
        Err(e) => {
            print_event(StreamEvent::error(e.to_string()));
            print_event(StreamEvent::done(None));
        }
    }
}

/// Handle a quick action request
async fn handle_quick_action(
    agent: &mut Agent,
    session_manager: &mut SessionManager,
    action: QuickActionType,
    code: String,
    context: Option<RequestContext>,
    instructions: Option<String>,
) {
    // Build prompt based on action type
    let prompt = build_quick_action_prompt(action, &code, &context, instructions);
    
    let result = agent.chat_stream_json(&prompt).await;
    
    match result {
        Ok(usage) => {
            // Save session state after successful action
            let stats = agent.token_stats();
            session_manager.set_messages(agent.messages().to_vec());
            session_manager.update_stats(stats.last_input_tokens, stats.total_output_tokens);
            if let Err(e) = session_manager.save_current() {
                print_event(StreamEvent::error(format!("Failed to save session: {}", e)));
            }
            print_event(StreamEvent::done(Some(usage)));
        }
        Err(e) => {
            print_event(StreamEvent::error(e.to_string()));
            print_event(StreamEvent::done(None));
        }
    }
}

/// Handle new session request
fn handle_new_session(
    agent: &mut Agent,
    session_manager: &mut SessionManager,
    project_root: Option<&std::path::Path>,
) {
    // Clear agent messages
    agent.clear_messages();
    
    // Start new session
    session_manager.clear_current().ok();
    session_manager.start_new(project_root).ok();
    
    let session_id = session_manager.current_metadata()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "new".to_string());
    
    print_event(StreamEvent::session_started(session_id, None));
}

/// Handle status request
fn handle_status(agent: &Agent, session_manager: &SessionManager) {
    let stats = agent.token_stats();
    
    let response = StreamEvent::StatusResponse {
        session_id: session_manager.current_metadata().map(|m| m.id.clone()),
        message_count: agent.messages().len(),
        total_tokens: stats.total_output_tokens + stats.last_input_tokens as u64,
        is_streaming: false,
    };
    
    print_event(response);
}

/// Handle memory operation
fn handle_memory(operation: crate::protocol::MemoryOperation) {
    // TODO: Implement memory operations
    match operation {
        crate::protocol::MemoryOperation::List => {
            print_event(StreamEvent::error("Memory list not implemented in daemon mode"));
        }
        crate::protocol::MemoryOperation::Search { query } => {
            print_event(StreamEvent::error(format!("Memory search not implemented: {}", query)));
        }
        crate::protocol::MemoryOperation::Add { content, category } => {
            print_event(StreamEvent::MemoryAdded {
                category: category.unwrap_or_else(|| "general".to_string()),
                content,
            });
        }
        crate::protocol::MemoryOperation::Clear => {
            print_event(StreamEvent::error("Memory clear not implemented in daemon mode"));
        }
        crate::protocol::MemoryOperation::Stats => {
            print_event(StreamEvent::MemoryStats {
                total: 0,
                by_category: std::collections::HashMap::new(),
            });
        }
    }
}

/// Handle load session request
fn handle_load_session(
    agent: &mut Agent,
    session_manager: &mut SessionManager,
    session_id: String,
    project_root: Option<&std::path::Path>,
) {
    match session_manager.resume(&session_id, project_root) {
        Ok(Some(session)) => {
            agent.set_messages(session.messages.clone());
            print_event(StreamEvent::session_started(session_id, None));
        }
        Ok(None) => {
            print_event(StreamEvent::error(format!("Session '{}' not found", session_id)));
        }
        Err(e) => {
            print_event(StreamEvent::error(format!("Failed to load session: {}", e)));
        }
    }
}

/// Handle list sessions request
fn handle_list_sessions(session_manager: &SessionManager) {
    let sessions: Vec<crate::protocol::SessionInfo> = session_manager.list_sessions()
        .iter()
        .map(|s| crate::protocol::SessionInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            created_at: s.created_at.to_rfc3339(),
            message_count: s.message_count,
            last_used: Some(s.updated_at.to_rfc3339()),
        })
        .collect();
    
    print_event(StreamEvent::SessionList { sessions });
}

/// Enhance message with context information
fn enhance_message_with_context(content: String, context: &RequestContext) -> String {
    let mut enhanced = content;
    
    // Add file context if available
    if let Some(ref file) = context.file {
        enhanced = format!(
            "Context: Current file is {} (language: {})\n\n{}",
            file,
            context.language.as_deref().unwrap_or("unknown"),
            enhanced
        );
    }
    
    // Add workspace context if available
    if let Some(ref workspace) = context.workspace {
        enhanced = format!(
            "Working in project: {}\n\n{}",
            workspace,
            enhanced
        );
    }
    
    enhanced
}

/// Build prompt for quick action
fn build_quick_action_prompt(
    action: QuickActionType,
    code: &str,
    context: &Option<RequestContext>,
    instructions: Option<String>,
) -> String {
    let language = context.as_ref()
        .and_then(|c| c.language.as_ref())
        .map(|l| l.as_str())
        .unwrap_or("");
    
    let action_prompt = match action {
        QuickActionType::Explain => "Explain this code in detail. What does it do? How does it work?",
        QuickActionType::Fix => "Fix any issues, bugs, or errors in this code. Provide the corrected version.",
        QuickActionType::GenerateTests => "Generate comprehensive unit tests for this code. Include edge cases.",
        QuickActionType::Refactor => "Refactor this code to improve readability, performance, or structure.",
        QuickActionType::Optimize => "Optimize this code for better performance. Explain the improvements.",
        QuickActionType::Document => "Add documentation/comments to this code. Include docstrings if appropriate.",
        QuickActionType::Translate => "Translate this code to another language (specify target language if needed).",
    };
    
    let mut prompt = format!(
        "{}\n\n```{}\n{}\n```",
        action_prompt,
        language,
        code
    );
    
    if let Some(ref instr) = instructions {
        prompt = format!("Additional instructions: {}\n\n{}", instr, prompt);
    }
    
    prompt
}

/// Print a stream event to stdout as JSON line
fn print_event(event: StreamEvent) {
    let json = event.to_json_line();
    let mut out = stdout();
    out.write_all(json.as_bytes()).ok();
    out.flush().ok();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_enhance_message_with_file() {
        let content = "Fix this code".to_string();
        let context = RequestContext {
            file: Some("src/main.rs".to_string()),
            language: Some("rust".to_string()),
            workspace: Some("/project".to_string()),
            ..Default::default()
        };
        
        let enhanced = enhance_message_with_context(content, &context);
        assert!(enhanced.contains("src/main.rs"));
        assert!(enhanced.contains("rust"));
        assert!(enhanced.contains("/project"));
    }
    
    #[test]
    fn test_build_quick_action_prompt_explain() {
        let code = "fn main() {}";
        let context = Some(RequestContext {
            language: Some("rust".to_string()),
            ..Default::default()
        });
        
        let prompt = build_quick_action_prompt(QuickActionType::Explain, code, &context, None);
        assert!(prompt.contains("Explain this code"));
        assert!(prompt.contains("```rust"));
        assert!(prompt.contains(code));
    }
    
    #[test]
    fn test_build_quick_action_prompt_with_instructions() {
        let code = "fn foo() {}";
        let instructions = Some("Make it async".to_string());
        
        let prompt = build_quick_action_prompt(QuickActionType::Refactor, code, &None, instructions);
        assert!(prompt.contains("Additional instructions: Make it async"));
    }
}