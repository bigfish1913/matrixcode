//! MatrixCode TUI - Terminal UI Entry Point

use anyhow::Result;
use clap::Parser;
use matrixcode_core::{
    AgentEvent, Config, cancel::CancellationToken,
    agent::AgentBuilder,
    AnthropicProvider,
    SessionManager,
    tools::all_tools_with_skills,
    memory::MemoryStorage,
};
use matrixcode::{TuiApp, setup_terminal, restore_terminal};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "matrixcode")]
#[command(about = "AI Code Agent with TUI interface")]
#[command(version)]
struct Cli {
    /// Continue last session
    #[arg(short, long)]
    continue_session: bool,

    /// Resume session by ID
    #[arg(long)]
    resume_id: Option<String>,

    /// Extra skills directory
    #[arg(long)]
    skills_dir: Option<PathBuf>,

    /// Think mode
    #[arg(long, default_value = "true")]
    think: bool,

    /// Max tokens
    #[arg(long, default_value = "16384")]
    max_tokens: u32,
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();
    
    let cli = Cli::parse();
    run_tui(cli)?;
    Ok(())
}

/// Load skills from directories
fn load_skills(extra_dirs: &[PathBuf]) -> Vec<matrixcode_core::skills::Skill> {
    use matrixcode_core::skills::discover_skills;

    let mut roots: Vec<PathBuf> = Vec::new();

    // Global skills (~/.matrix/skills)
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".matrix").join("skills"));
    }

    // Project-local skills
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join(".matrix").join("skills"));
        roots.push(cwd.join("skills"));
    }

    roots.extend(extra_dirs.iter().cloned());

    discover_skills(&roots)
}

/// Run TUI mode
fn run_tui(cli: Cli) -> Result<()> {
    let config = Config::load();

    // Get API configuration
    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("No API key found. Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json"))?;

    let model = config.model.clone()
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let base_url = config.base_url.clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());

    // Load skills
    let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
    let skills = load_skills(&skills_dirs);

    // Setup tokio runtime
    let rt = tokio::runtime::Runtime::new()?;

    // Create channels
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
    let (task_tx, mut task_rx) = tokio::sync::mpsc::channel::<String>(10);
    let (ask_tx, ask_rx) = tokio::sync::mpsc::channel::<String>(1);

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Load session
    let project_path = std::env::current_dir().ok();
    let (restored_messages, session_mgr_state) = {
        let mut mgr = SessionManager::new().ok();
        let mut messages = Vec::new();
        
        if let Some(ref mut mgr) = mgr {
            if cli.continue_session || cli.resume_id.is_some() {
                let session = if let Some(ref query) = cli.resume_id {
                    mgr.resume(query, project_path.as_deref()).ok().flatten()
                } else {
                    mgr.continue_last(project_path.as_deref()).ok().flatten()
                };
                if let Some(s) = session {
                    messages = s.messages.clone();
                }
            } else {
                let _ = mgr.start_new(project_path.as_deref());
            }
        }
        (messages, mgr)
    };

    // Clone for agent task
    let agent_cancel = cancel_token.clone();
    let agent_event_tx = event_tx.clone();
    let agent_api_key = api_key.clone();
    let agent_model = model.clone();
    let agent_base_url = base_url.clone();
    let agent_think = cli.think;
    let agent_max_tokens = cli.max_tokens;
    let agent_restored_messages = restored_messages.clone();
    let agent_project_path = project_path.clone();
    let agent_approve_mode = config.approve_mode.as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);
    let agent_skills = skills.clone();
    
    let shared_approve_mode = Arc::new(std::sync::atomic::AtomicU8::new(agent_approve_mode.to_u8()));
    let agent_shared_approve_mode = shared_approve_mode.clone();

    // Spawn agent task
    let _agent_task = rt.spawn(async move {
        let provider = AnthropicProvider::new(agent_api_key.clone(), agent_model.clone(), agent_base_url.clone());

        // Load memory
        let project_path_ref = agent_project_path.as_deref();
        let mut memory_storage = MemoryStorage::new(project_path_ref).ok();
        let memory = memory_storage.as_ref()
            .and_then(|ms| ms.load_combined().ok());
        
        if let Some(ref mem) = memory && !mem.entries.is_empty() {
            let _ = agent_event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::MemoryLoaded,
                matrixcode_core::EventData::Memory {
                    summary: mem.generate_prompt_summary(10),
                    entries_count: mem.entries.len(),
                },
            )).await;
        }
        
        let initial_memory_summary = memory.as_ref()
            .map(|mem| mem.generate_prompt_summary(20))
            .unwrap_or_default();

        // Load project overview
        let project_overview = project_path_ref
            .and_then(|path| matrixcode_core::overview::ProjectOverview::load(path).ok().flatten());

        // Build system prompt
        let system_prompt = matrixcode_core::prompt::build_system_prompt(
            &matrixcode_core::prompt::PromptProfile::Default,
            &agent_skills,
            project_overview.as_ref().map(|o| o.content.as_str()),
            if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
        );

        // Build agent
        let mut agent = AgentBuilder::new(Box::new(provider))
            .system_prompt(system_prompt)
            .model_name(agent_model.clone())
            .max_tokens(agent_max_tokens)
            .think(agent_think)
            .tools(all_tools_with_skills(Arc::new(agent_skills.clone())))
            .event_tx(agent_event_tx.clone())
            .approve_mode(agent_approve_mode)
            .build();

        agent.set_approve_mode_shared(agent_shared_approve_mode);

        if !agent_restored_messages.is_empty() {
            agent.set_messages(agent_restored_messages);
        }

        let mut session_mgr = session_mgr_state;
        agent.set_cancel_token(agent_cancel.clone());
        agent.set_ask_channel(ask_rx);

        while let Some(msg) = task_rx.recv().await {
            if agent_cancel.is_cancelled() {
                agent_event_tx.send(AgentEvent::error(
                    "Operation interrupted by user".to_string(),
                    Some("interrupted".to_string()),
                    None,
                )).await.ok();
                agent_cancel.reset();
                continue;
            }

            // Handle /new command
            if msg == "/new" {
                agent.clear_history();
                if let Some(ref mut mgr) = session_mgr {
                    let _ = mgr.start_new(agent_project_path.as_deref());
                }
                let _ = agent_event_tx.send(AgentEvent::session_ended()).await;
                continue;
            }

            // Dynamic memory retrieval
            if let Some(ref mem) = memory {
                let context_keywords = matrixcode_core::memory::extract_context_keywords(&msg);
                let contextual_summary = mem.generate_contextual_summary_with_keywords(&context_keywords, 15);
                
                if !contextual_summary.is_empty() {
                    agent.update_memory_summary(Some(contextual_summary));
                }
            }

            // Run agent
            match agent.run(msg.clone()).await {
                Ok(_) => {
                    // Auto-save session
                    if let Some(ref mut mgr) = session_mgr {
                        let (input_tokens, output_tokens) = agent.get_token_counts();
                        let messages = agent.get_messages();
                        mgr.set_messages(messages.to_vec());
                        mgr.update_stats(input_tokens as u32, output_tokens);
                        let _ = mgr.save_current();
                    }
                    
                    // Auto-detect memories
                    if let Some(ref mut ms) = memory_storage {
                        let messages = agent.get_messages();
                        if let Some(last_msg) = messages.last() {
                            let text = match &last_msg.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks.iter().filter_map(|b| match b {
                                        matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                        _ => None,
                                    }).collect::<Vec<_>>().join("\n")
                                }
                            };

                            let detected = matrixcode_core::memory::detect_memories_from_text(&text, None);
                            if !detected.is_empty() {
                                let detected_count = detected.len();
                                if let Ok(mut mem) = ms.load_global() {
                                    for entry in detected {
                                        mem.add(entry);
                                    }
                                    let _ = ms.save_global(&mem);
                                    
                                    let _ = agent_event_tx.send(AgentEvent::with_data(
                                        matrixcode_core::EventType::MemoryDetected,
                                        matrixcode_core::EventData::Memory {
                                            summary: format!("Detected {} memory entries", detected_count),
                                            entries_count: detected_count,
                                        },
                                    )).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    agent_event_tx.send(AgentEvent::error(
                        format!("Agent error: {}", e),
                        Some("agent_error".to_string()),
                        None,
                    )).await.ok();
                }
            }
        }
    });

    let _guard = rt.enter();

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Create and run TUI app
    let mut app = TuiApp::new(task_tx, event_rx, cancel_token)
        .with_ask_channel(ask_tx)
        .with_shared_approve_mode(shared_approve_mode)
        .with_config(&model, cli.think, cli.max_tokens, None);
    
    if !restored_messages.is_empty() {
        app.load_messages(restored_messages);
    }
    
    let result = app.run(&mut terminal);

    restore_terminal()?;

    result
}