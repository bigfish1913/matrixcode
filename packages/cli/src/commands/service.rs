//! Service mode execution
//!
//! Handles CLI commands without TUI, with text or JSON output.

use anyhow::Result;
use matrixcode_core::{
    AgentEvent, AgentBuilder, Config,
    create_provider_with_headers,
    providers::{MessageContent, Role, ContentBlock},
    tools::all_tools_full,
    prompt::{build_system_prompt_with_workflows, PromptProfile, preprocess_with_skills, ProcessResult},
    approval::ApproveMode,
    session::SessionManager,
    memory::MemoryStorage,
    skills::Skill,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::constants::{EVENT_CHANNEL_BUFFER, QUICK_ACTION_MAX_TOKENS, EVENT_TIMEOUT_MS, DISPLAY_SESSIONS_LIMIT};
use crate::types::{Cli, Commands};
use crate::helpers::{resolve_provider, resolve_model, resolve_base_url, model_with_source, load_skills};
use crate::commands::handle_workflow_command;
use crate::display::{print_response_border, print_thinking_border};

/// Handle single command with actual agent execution
pub fn handle_command(cmd: Commands, skills: &[Skill]) {
    let config = Config::load();

    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .unwrap_or_else(|| {
            eprintln!("❌ No API key found. Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json");
            std::process::exit(1);
        });

    let model = resolve_model(&config);
    let base_url = resolve_base_url(&config);

    let approve_mode = config
        .approve_mode
        .as_ref()
        .map(|m| ApproveMode::parse(m))
        .unwrap_or(ApproveMode::Ask);

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create tokio runtime: {}", e);
            return;
        }
    };

    rt.block_on(async {
        match cmd {
            Commands::Chat { message } => {
                if let Some(msg) = message {
                    handle_chat(&config, &api_key, &model, &base_url, skills, approve_mode, msg).await;
                } else {
                    println!("Starting interactive chat session...");
                    println!("Note: For interactive chat, run 'matrixcode' without subcommand.");
                }
            }
            Commands::Status => handle_status(&config, &model),
            Commands::History => handle_history(),
            Commands::NewSession => handle_new_session(),
            Commands::QuickAction { action, file } => {
                handle_quick_action(&config, &api_key, &model, &base_url, skills, action, file).await;
            }
            Commands::Workflow { command } => {
                handle_workflow_command(command);
            }
        }
    });
}

/// Service mode: pure JSON output
pub fn run_service_mode(cli: Cli) -> Result<()> {
    let config = Config::load();

    match cli.command {
        Some(Commands::Chat { message }) => {
            let api_key = config.api_key.clone()
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

            let model = resolve_model(&config);
            let base_url = resolve_base_url(&config);

            let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
            let skills = load_skills(&skills_dirs);

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                println!("{}", AgentEvent::session_started().to_json()?);

                if let Some(msg) = message {
                    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(EVENT_CHANNEL_BUFFER);

                    let project_path = std::env::current_dir().unwrap_or_default();
                    let system_prompt = build_system_prompt_with_workflows(&PromptProfile::Default, &skills, None, None, Some(&project_path), None);

                    let provider = match create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            println!("{}", AgentEvent::error(format!("Failed to create provider: {}", e), None, None).to_json()?);
                            return Ok(())
                        }
                    };

                    let mut agent = AgentBuilder::new(provider.clone_box())
                        .system_prompt(system_prompt)
                        .model_name(model)
                        .max_tokens(QUICK_ACTION_MAX_TOKENS)
                        .tools(all_tools_full(
                            Arc::new(skills.clone()),
                            provider.clone_arc(),
                            project_path,
                        ))
                        .approve_mode(ApproveMode::Auto)
                        .event_tx(event_tx)
                        .build();

                    // Pre-process: detect skill/workflow triggers with skills
                    let processed_msg = match preprocess_with_skills(&msg, &skills) {
                        ProcessResult::SkillTriggered { skill_id, confidence: _, skill_body } => {
                            if let Some(body) = skill_body {
                                format!(
                                    "# Skill: {}\n\n{}\n\n---\n\n用户原始请求：{}",
                                    skill_id, body, msg
                                )
                            } else {
                                format!(
                                    "【系统检测到应使用技能: {}】\n\n请先调用 skill 工具加载此技能，然后立即执行其中的指令。\n\n用户原始请求：{}",
                                    skill_id, msg
                                )
                            }
                        }
                        ProcessResult::WorkflowTriggered { workflow_id, inputs } => {
                            let inputs_json = serde_json::to_string(&inputs).unwrap_or_default();
                            format!(
                                "【系统检测到应使用工作流: {}】\n\n请先调用 workflow_run 工具执行此工作流，参数如下：{}\n\n用户原始请求：{}",
                                workflow_id, inputs_json, msg
                            )
                        }
                        ProcessResult::Continue => msg.clone(),
                    };

                    let run_result = agent.run(processed_msg).await;

                    while let Some(event) = event_rx.recv().await {
                        match event.event_type {
                            matrixcode_core::EventType::TextDelta | matrixcode_core::EventType::Error => {
                                println!("{}", event.to_json()?);
                            }
                            matrixcode_core::EventType::SessionEnded => break,
                            _ => {}
                        }
                    }

                    match run_result {
                        Ok(_) => println!("{}", AgentEvent::session_ended().to_json()?),
                        Err(e) => println!("{}", AgentEvent::error(format!("Error: {}", e), None, None).to_json()?),
                    }
                }

                Ok(())
            })
        }
        Some(Commands::History) => {
            println!("{}", AgentEvent::session_started().to_json()?);
            handle_history_json()?;
            println!("{}", AgentEvent::session_ended().to_json()?);
            Ok(())
        }
        Some(Commands::Status) => {
            println!("{}", AgentEvent::session_started().to_json()?);
            handle_status_json(&config, &resolve_model(&config))?;
            println!("{}", AgentEvent::session_ended().to_json()?);
            Ok(())
        }
        Some(Commands::NewSession) => {
            println!("{}", AgentEvent::session_started().to_json()?);
            handle_new_session_json()?;
            println!("{}", AgentEvent::session_ended().to_json()?);
            Ok(())
        }
        Some(Commands::QuickAction { action, file }) => {
            let api_key = config.api_key.clone()
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

            let model = resolve_model(&config);
            let base_url = resolve_base_url(&config);

            let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
            let skills = load_skills(&skills_dirs);

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                println!("{}", AgentEvent::session_started().to_json()?);
                handle_quick_action_json(&config, &api_key, &model, &base_url, &skills, action, file).await?;
                println!("{}", AgentEvent::session_ended().to_json()?);
                Ok(())
            })
        }
        Some(Commands::Workflow { command }) => {
            println!("{}", AgentEvent::session_started().to_json()?);
            handle_workflow_command(command);
            println!("{}", AgentEvent::session_ended().to_json()?);
            Ok(())
        }
        None => {
            println!("{}", AgentEvent::error("Please specify a command".to_string(), None, None).to_json()?);
            Ok(())
        }
    }
}

// === Terminal output handlers ===

async fn handle_chat(
    config: &Config,
    api_key: &str,
    model: &str,
    base_url: &str,
    skills: &[Skill],
    approve_mode: ApproveMode,
    msg: String,
) {
    let project_path = std::env::current_dir().unwrap_or_default();
    let system_prompt = build_system_prompt_with_workflows(&PromptProfile::Default, skills, None, None, Some(&project_path), None);

    let provider = match create_provider_with_headers(
        resolve_provider(config, model),
        api_key.to_string(),
        model.to_string(),
        Some(base_url.to_string()),
        config.extra_headers.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create provider: {}", e);
            return;
        }
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(EVENT_CHANNEL_BUFFER);

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(model.to_string())
        .max_tokens(QUICK_ACTION_MAX_TOKENS)
        .tools(all_tools_full(
            Arc::new(skills.to_vec()),
            provider.clone_arc(),
            project_path,
        ))
        .approve_mode(approve_mode)
        .event_tx(event_tx)
        .proxy_executor(
            matrixcode_tui::image_search::create_default_executor(),
            matrixcode_tui::image_search::get_default_proxy_tools()
        )
        .build();

    // Pre-process: detect skill/workflow triggers with skills
    let processed_msg = match preprocess_with_skills(&msg, &skills) {
        ProcessResult::SkillTriggered { skill_id, confidence: _, skill_body } => {
            if let Some(body) = skill_body {
                format!(
                    "# Skill: {}\n\n{}\n\n---\n\n用户原始请求：{}",
                    skill_id, body, msg
                )
            } else {
                format!(
                    "【系统检测到应使用技能: {}】\n\n请先调用 skill 工具加载此技能，然后立即执行其中的指令。\n\n用户原始请求：{}",
                    skill_id, msg
                )
            }
        }
        ProcessResult::WorkflowTriggered { workflow_id, inputs } => {
            let inputs_json = serde_json::to_string(&inputs).unwrap_or_default();
            format!(
                "【系统检测到应使用工作流: {}】\n\n请先调用 workflow_run 工具执行此工作流，参数如下：{}\n\n用户原始请求：{}",
                workflow_id, inputs_json, msg
            )
        }
        ProcessResult::Continue => msg.clone(),
    };

    let run_future = agent.run(processed_msg);
    let event_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if event.event_type == matrixcode_core::EventType::Error
                && let Some(data) = &event.data {
                    eprintln!("⚠️ Error event: {:?}", data);
                }
        }
    });

    let result = run_future.await;
    let _ = tokio::time::timeout(tokio::time::Duration::from_millis(EVENT_TIMEOUT_MS), event_task).await;

    match result {
        Ok(_) => {
            show_agent_response(&agent);
            let (input, output) = agent.get_token_counts();
            println!();
            println!("📊 Tokens: {} in, {} out", input, output);
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

async fn handle_quick_action(
    config: &Config,
    api_key: &str,
    model: &str,
    base_url: &str,
    skills: &[Skill],
    action: String,
    file: Option<String>,
) {
    println!("⚡ Quick Action: {}", action);
    if let Some(f) = &file {
        println!("  Target: {}", f);
    }

    let project_path = std::env::current_dir().unwrap_or_default();
    let prompt = build_action_prompt(&action, &file);
    let system_prompt = build_system_prompt_with_workflows(&PromptProfile::Fast, skills, None, None, Some(&project_path), None);

    let provider = match create_provider_with_headers(
        resolve_provider(config, model),
        api_key.to_string(),
        model.to_string(),
        Some(base_url.to_string()),
        config.extra_headers.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create provider: {}", e);
            return;
        }
    };

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(model.to_string())
        .max_tokens(QUICK_ACTION_MAX_TOKENS)
        .tools(all_tools_full(
            Arc::new(skills.to_vec()),
            provider.clone_arc(),
            project_path,
        ))
        .approve_mode(ApproveMode::Auto)
        .build();

    // Pre-process: detect skill/workflow triggers with skills
    let processed_prompt = match preprocess_with_skills(&prompt, &skills) {
        ProcessResult::SkillTriggered { skill_id, confidence: _, skill_body } => {
            if let Some(body) = skill_body {
                format!(
                    "# Skill: {}\n\n{}\n\n---\n\n用户原始请求：{}",
                    skill_id, body, prompt
                )
            } else {
                format!(
                    "【系统检测到应使用技能: {}】\n\n请先调用 skill 工具加载此技能，然后立即执行其中的指令。\n\n用户原始请求：{}",
                    skill_id, prompt
                )
            }
        }
        ProcessResult::WorkflowTriggered { workflow_id, inputs } => {
            let inputs_json = serde_json::to_string(&inputs).unwrap_or_default();
            format!(
                "【系统检测到应使用工作流: {}】\n\n请先调用 workflow_run 工具执行此工作流，参数如下：{}\n\n用户原始请求：{}",
                workflow_id, inputs_json, prompt
            )
        }
        ProcessResult::Continue => prompt.clone(),
    };

    match agent.run(processed_prompt).await {
        Ok(_) => {
            show_agent_response(&agent);
            let (input, output) = agent.get_token_counts();
            println!();
            println!("📊 Tokens: {} in, {} out", input, output);
            println!("✓ Action completed");
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

fn handle_status(config: &Config, _model: &str) {
    println!("MatrixCode Status:\n");
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));
    println!("  Mode: Ready");

    if config.is_api_configured() {
        println!("  API: ✓ configured");
    } else {
        println!("  API: ❌ not configured");
        println!("       Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json");
    }

    println!("  Model: {}", model_with_source(config));

    if let Some(base_url) = &config.base_url {
        println!("  Base URL: {}", base_url);
    } else if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
        println!("  Base URL: {} (from env)", url);
    }

    if let Some(mode) = &config.approve_mode {
        println!("  Approve Mode: {}", mode);
    } else {
        println!("  Approve Mode: ask (default)");
    }

    if let Ok(mgr) = SessionManager::new() {
        println!("  Sessions: {} (current: {})",
            mgr.list_sessions().len(),
            if mgr.has_current() { "yes" } else { "no" }
        );
    }

    let project_path = std::env::current_dir().ok();
    if let Some(path) = &project_path {
        if let Ok(storage) = MemoryStorage::new(Some(path.as_path()))
            && let Ok(mem) = storage.load_combined() {
                println!("  Memory: {} entries", mem.entries.len());
            }

        let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
        if overview_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&overview_path) {
                let size = metadata.len();
                if let Ok(modified) = metadata.modified() {
                    let modified_time: chrono::DateTime<chrono::Local> = modified.into();
                    println!("  Overview: ✓ MATRIX.md ({}, modified: {})",
                        if size > 1024 { format!("{} KB", size / 1024) } else { format!("{} bytes", size) },
                        modified_time.format("%Y-%m-%d %H:%M")
                    );
                } else {
                    println!("  Overview: ✓ MATRIX.md ({})", size);
                }
            }
        } else {
            println!("  Overview: ❌ not found (use /init to generate)");
        }
    }
}

fn handle_history() {
    if let Ok(mgr) = SessionManager::new() {
        let sessions = mgr.list_sessions();
        if sessions.is_empty() {
            println!("No session history found.");
        } else {
            println!("Session History:\n");
            for session in sessions {
                let project = session.project_path.as_deref().unwrap_or("unknown");
                let is_current = mgr.has_current() && mgr.current_id() == Some(session.id.as_str());

                println!("Session: {} ({})", session.short_id(), session.id);
                println!("  Project: {}", project);
                println!("  Created: {}", session.created_at.format("%Y-%m-%d %H:%M"));
                println!("  Current: {}", if is_current { "yes" } else { "no" });
                println!("  Messages: {}", session.message_count);
                println!("  Tokens: {} in, {} out", session.last_input_tokens, session.total_output_tokens);
                println!();
            }
            println!("Total: {} sessions", sessions.len());
            println!("\nResume: matrixcode --resume <id>");
        }
    } else {
        println!("Session manager not available.");
    }
}

fn handle_new_session() {
    println!("Creating new session...");

    if let Ok(mut mgr) = SessionManager::new() {
        let project_path = std::env::current_dir().ok();
        if mgr.start_new(project_path.as_deref()).is_ok() {
            println!("✓ New session created");
            if let Some(id) = mgr.current_id() {
                println!("  Session ID: {}", id);
            }
            println!("\nStart chatting with: matrixcode");
        } else {
            println!("❌ Failed to create new session");
        }
    } else {
        println!("Session manager not available.");
    }
}

// === JSON output handlers ===

fn handle_history_json() -> Result<()> {
    if let Ok(mgr) = SessionManager::new() {
        let sessions = mgr.list_sessions();
        for session in sessions.iter().take(DISPLAY_SESSIONS_LIMIT) {
            println!("{}", AgentEvent::progress(
                format!("{} - {} ({} msgs)", session.short_id(), session.id, session.message_count),
                None,
            ).to_json()?);
        }
    }
    Ok(())
}

fn handle_status_json(config: &Config, _model: &str) -> Result<()> {
    println!("{}", AgentEvent::progress(
        format!("Model: {}\nAPI: {}\nSessions: {}",
            model_with_source(config),
            if config.is_api_configured() { "configured" } else { "not configured" },
            SessionManager::new().map(|m| m.list_sessions().len()).unwrap_or(0)
        ),
        None,
    ).to_json()?);
    Ok(())
}

fn handle_new_session_json() -> Result<()> {
    if let Ok(mut mgr) = SessionManager::new() {
        let project_path = std::env::current_dir().ok();
        if mgr.start_new(project_path.as_deref()).is_ok() {
            println!("{}", AgentEvent::progress(
                format!("New session created: {}", mgr.current_id().unwrap_or("unknown")),
                None,
            ).to_json()?);
        }
    }
    Ok(())
}

async fn handle_quick_action_json(
    config: &Config,
    api_key: &str,
    model: &str,
    base_url: &str,
    skills: &[Skill],
    action: String,
    file: Option<String>,
) -> Result<()> {
    let project_path = std::env::current_dir().unwrap_or_default();
    let prompt = build_action_prompt(&action, &file);
    let system_prompt = build_system_prompt_with_workflows(&PromptProfile::Fast, skills, None, None, Some(&project_path), None);

    let provider = create_provider_with_headers(
        resolve_provider(config, model),
        api_key.to_string(),
        model.to_string(),
        Some(base_url.to_string()),
        config.extra_headers.clone(),
    )?;

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(model.to_string())
        .max_tokens(QUICK_ACTION_MAX_TOKENS)
        .tools(all_tools_full(
            Arc::new(skills.to_vec()),
            provider.clone_arc(),
            project_path,
        ))
        .approve_mode(ApproveMode::Auto)
        .build();

    // Pre-process: detect skill/workflow triggers with skills
    let processed_prompt = match preprocess_with_skills(&prompt, &skills) {
        ProcessResult::SkillTriggered { skill_id, confidence: _, skill_body } => {
            if let Some(body) = skill_body {
                format!(
                    "# Skill: {}\n\n{}\n\n---\n\n用户原始请求：{}",
                    skill_id, body, prompt
                )
            } else {
                format!(
                    "【系统检测到应使用技能: {}】\n\n请先调用 skill 工具加载此技能，然后立即执行其中的指令。\n\n用户原始请求：{}",
                    skill_id, prompt
                )
            }
        }
        ProcessResult::WorkflowTriggered { workflow_id, inputs } => {
            let inputs_json = serde_json::to_string(&inputs).unwrap_or_default();
            format!(
                "【系统检测到应使用工作流: {}】\n\n请先调用 workflow_run 工具执行此工作流，参数如下：{}\n\n用户原始请求：{}",
                workflow_id, inputs_json, prompt
            )
        }
        ProcessResult::Continue => prompt.clone(),
    };

    agent.run(processed_prompt).await?;
    println!("{}", AgentEvent::progress("Action completed".to_string(), None).to_json()?);

    Ok(())
}

// === Helper functions ===

fn build_action_prompt(action: &str, file: &Option<String>) -> String {
    match action {
        "explain" => {
            if let Some(f) = file {
                format!("Please explain the code in {} in detail.", f)
            } else {
                "Please explain the code in detail.".to_string()
            }
        }
        "fix" => {
            if let Some(f) = file {
                format!("Please analyze {} for bugs and fix them.", f)
            } else {
                "Please analyze the code for bugs and fix them.".to_string()
            }
        }
        "refactor" => {
            if let Some(f) = file {
                format!("Please refactor {} to improve structure.", f)
            } else {
                "Please refactor the code to improve structure.".to_string()
            }
        }
        "test" => {
            if let Some(f) = file {
                format!("Please write unit tests for {}.", f)
            } else {
                "Please write unit tests for the code.".to_string()
            }
        }
        "doc" | "document" => {
            if let Some(f) = file {
                format!("Please add documentation to {}.", f)
            } else {
                "Please add documentation to the code.".to_string()
            }
        }
        "optimize" => {
            if let Some(f) = file {
                format!("Please optimize {} for performance.", f)
            } else {
                "Please optimize the code for performance.".to_string()
            }
        }
        "review" => {
            if let Some(f) = file {
                format!("Please review {} and provide feedback.", f)
            } else {
                "Please review the code and provide feedback.".to_string()
            }
        }
        other => {
            if let Some(f) = file {
                format!("{}: {}", other, f)
            } else {
                other.to_string()
            }
        }
    }
}

fn show_agent_response(agent: &matrixcode_core::Agent) {
    let messages = agent.get_messages();

    // Show thinking content if any
    for msg in messages.iter() {
        if msg.role == Role::Assistant {
            let is_thinking = match &msg.content {
                MessageContent::Text(t) => t.contains("<thinking>") || t.starts_with("Let me"),
                MessageContent::Blocks(blocks) => blocks.iter().any(|b| matches!(b, ContentBlock::Thinking { .. })),
            };

            if is_thinking {
                let text = extract_message_text(&msg.content);
                print_thinking_border(&text);
            }
        }
    }

    // Show final response
    if let Some(last) = messages.last() && last.role == Role::Assistant {
        let text = extract_message_text(&last.content);
        print_response_border("Response", &text);
    }
}

fn extract_message_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}