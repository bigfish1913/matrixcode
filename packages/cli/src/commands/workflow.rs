//! Workflow command handling

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use matrixcode_core::config::Config;
use matrixcode_core::providers::ProviderType;
use matrixcode_core::workflow::{
    parse_workflow_from_file, WorkflowEngine, WorkflowPersistence, WorkflowRegistry,
    WorkflowStatus, WorkflowSource,
};
use matrixcode_core::workflow::executors::ExecutorFactory;
use matrixcode_core::{create_provider_with_headers, infer_provider_type};

use crate::CliArgs;

/// Workflow subcommands
#[derive(clap::Subcommand, Debug)]
pub enum WorkflowCommands {
    /// Run a workflow from file
    Run {
        /// Workflow YAML file path
        file: String,
        /// JSON inputs for workflow
        #[arg(long)]
        inputs: Option<String>,
    },
    /// Discover available workflows
    Discover {
        /// Query to match workflows
        query: Option<String>,
    },
    /// List workflow history
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
    },
    /// Show workflow status
    Status {
        /// Workflow instance ID
        id: String,
    },
    /// Resume a paused workflow
    Resume {
        /// Workflow instance ID
        id: String,
    },
    /// Abort a running workflow
    Abort {
        /// Workflow instance ID
        id: String,
    },
    /// Export workflow diagram
    Export {
        /// Workflow YAML file
        file: String,
        /// Export format (mermaid)
        #[arg(long, default_value = "mermaid")]
        format: String,
        /// Output file path
        #[arg(long)]
        output: Option<String>,
    },
}

/// Handle workflow subcommands
pub fn handle_workflow_command(command: WorkflowCommands, args: &CliArgs) {
    match command {
        WorkflowCommands::Run { file, inputs } => handle_run(file, inputs, args),
        WorkflowCommands::Discover { query } => handle_discover(query),
        WorkflowCommands::List { status } => handle_list(status),
        WorkflowCommands::Status { id } => handle_status(id),
        WorkflowCommands::Resume { id } => handle_resume(id),
        WorkflowCommands::Abort { id } => handle_abort(id),
        WorkflowCommands::Export { file, format, output } => handle_export(file, format, output),
    }
}

fn handle_run(file: String, inputs: Option<String>, args: &CliArgs) {
    println!("🔄 Running workflow from: {}", file);

    let workflow_def = match parse_workflow_from_file(&file) {
        Ok(def) => def,
        Err(e) => {
            eprintln!("❌ Failed to parse workflow: {}", e);
            return;
        }
    };

    println!("  Workflow: {}", workflow_def.id);
    println!("  Name: {}", workflow_def.name);
    println!("  Nodes: {}", workflow_def.nodes.len());

    let inputs_map: HashMap<String, serde_json::Value> = inputs
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let config = Config::load();
    let provider = create_provider(&config, args);

    let factory = if let Some(p) = provider {
        ExecutorFactory::new().with_provider(p)
    } else {
        ExecutorFactory::new()
    };

    let proxy_executor = matrixcode_tui::image_search::create_default_executor();
    let proxy_tool_defs = matrixcode_tui::image_search::get_default_proxy_tools();

    let engine = match WorkflowEngine::new(workflow_def) {
        Ok(e) => e
            .with_executor_factory(factory)
            .with_proxy_executor(proxy_executor, proxy_tool_defs),
        Err(e) => {
            eprintln!("❌ Failed to create workflow engine: {}", e);
            return;
        }
    };

    let context = run_workflow_async(engine, inputs_map);

    match context {
        Ok(ctx) => {
            println!();
            println!("📊 Workflow completed:");
            println!("  Instance ID: {}", ctx.instance_id);
            println!("  Status: {:?}", ctx.status);
            println!("  Nodes executed: {}", ctx.execution_path.len());

            if ctx.status == WorkflowStatus::Completed {
                println!("✓ Workflow completed successfully");
            } else if ctx.status == WorkflowStatus::Failed {
                println!("❌ Workflow failed: {}", ctx.error.as_ref().unwrap_or(&String::new()));
            }

            save_workflow_context(&ctx);
        }
        Err(e) => eprintln!("❌ Workflow execution failed: {}", e),
    }
}

fn handle_discover(query: Option<String>) {
    let project_path = std::env::current_dir().ok();
    let registry = WorkflowRegistry::new(project_path.as_ref());

    if registry.is_empty() {
        println!("No workflows found in:");
        println!("  - Project: .matrix/workflows/");
        println!("  - User: ~/.matrix/workflows/");
        println!("\nCreate workflow YAML files in these directories.");
        return;
    }

    if let Some(q) = query {
        let matches = registry.match_workflows(&q);
        if matches.is_empty() {
            println!("No workflows match query: '{}'", q);
            println!("\nAvailable workflows:");
            for info in registry.list() {
                let source = source_label(&info.source);
                println!("  - {} ({})", info.id, source);
            }
        } else {
            println!("🔍 Matching workflows for '{}':\n", q);
            for info in matches {
                print_workflow_info(&info);
            }
        }
    } else {
        println!("🔍 Discovered workflows ({}):\n", registry.count());
        println!("{}", registry.generate_summary());
    }
}

fn handle_list(status: Option<String>) {
    let project_path = std::env::current_dir().ok();
    let persistence = WorkflowPersistence::new(project_path.as_ref());

    let workflows = if let Some(filter) = status {
        let filter_status = parse_status_filter(&filter);
        persistence.list_by_status(filter_status).unwrap_or_default()
    } else {
        persistence.list().unwrap_or_default()
    };

    if workflows.is_empty() {
        println!("No workflows found.");
        println!("\nWorkflows are stored in:");
        println!("  - Project: .matrix/workflows/");
        println!("  - User: ~/.matrix/workflows/");
    } else {
        println!("📚 Workflow History:\n");
        for ctx in &workflows {
            println!("  {} - {} ({:?})", ctx.instance_id, ctx.workflow_id, ctx.status);
            println!("    Nodes: {} | Created: {}", ctx.execution_path.len(), ctx.created_at.format("%Y-%m-%d %H:%M"));
            if let Some(err) = &ctx.error {
                println!("    Error: {}", err.chars().take(50).collect::<String>());
            }
            println!();
        }
        println!("Total: {} workflows", workflows.len());
    }
}

fn handle_status(id: String) {
    let project_path = std::env::current_dir().ok();
    let persistence = WorkflowPersistence::new(project_path.as_ref());

    match persistence.load(&id) {
        Ok(Some(ctx)) => {
            println!("📊 Workflow Status:\n");
            println!("  Instance ID: {}", ctx.instance_id);
            println!("  Workflow: {}", ctx.workflow_id);
            println!("  Status: {:?}", ctx.status);
            println!("  Current Node: {}", ctx.current_node_id.as_ref().unwrap_or(&"none".to_string()));
            println!("  Created: {}", ctx.created_at.format("%Y-%m-%d %H:%M"));
            if let Some(started) = ctx.started_at {
                println!("  Started: {}", started.format("%Y-%m-%d %H:%M"));
            }
            if let Some(finished) = ctx.finished_at {
                println!("  Finished: {}", finished.format("%Y-%m-%d %H:%M"));
                if let Some(duration) = ctx.total_duration_ms() {
                    println!("  Duration: {} ms", duration);
                }
            }
            println!();
            println!("  Execution Path:");
            for node_id in &ctx.execution_path {
                if let Some(exec) = ctx.get_node_execution(node_id) {
                    println!("    - {} ({:?})", node_id, exec.status);
                }
            }
            if let Some(err) = &ctx.error {
                println!();
                println!("  ❌ Error: {}", err);
            }
        }
        Ok(None) => println!("❌ Workflow '{}' not found", id),
        Err(e) => eprintln!("❌ Failed to load workflow: {}", e),
    }
}

fn handle_resume(id: String) {
    println!("🔄 Resuming workflow: {}", id);

    let project_path = std::env::current_dir().ok();
    let persistence = WorkflowPersistence::new(project_path.as_ref());

    match persistence.load(&id) {
        Ok(Some(ctx)) => {
            if ctx.status != WorkflowStatus::Paused {
                println!("❌ Workflow is not paused (status: {:?})", ctx.status);
                return;
            }

            let mut ctx = ctx;
            ctx.resume();

            if let Err(e) = persistence.save(&ctx) {
                eprintln!("❌ Failed to save resumed workflow: {}", e);
            } else {
                println!("✓ Workflow resumed (status: Running)");
            }
        }
        Ok(None) => println!("❌ Workflow '{}' not found", id),
        Err(e) => eprintln!("❌ Failed to load workflow: {}", e),
    }
}

fn handle_abort(id: String) {
    println!("⏹️ Aborting workflow: {}", id);

    let project_path = std::env::current_dir().ok();
    let persistence = WorkflowPersistence::new(project_path.as_ref());

    match persistence.load(&id) {
        Ok(Some(ctx)) => {
            if ctx.status != WorkflowStatus::Running {
                println!("❌ Workflow is not running (status: {:?})", ctx.status);
                return;
            }

            let mut ctx = ctx;
            ctx.cancel();

            if let Err(e) = persistence.save(&ctx) {
                eprintln!("❌ Failed to save aborted workflow: {}", e);
            } else {
                println!("✓ Workflow aborted");
            }
        }
        Ok(None) => println!("❌ Workflow '{}' not found", id),
        Err(e) => eprintln!("❌ Failed to load workflow: {}", e),
    }
}

fn handle_export(file: String, format: String, output: Option<String>) {
    println!("📤 Exporting workflow from: {}", file);

    let workflow_def = match parse_workflow_from_file(&file) {
        Ok(def) => def,
        Err(e) => {
            eprintln!("❌ Failed to parse workflow: {}", e);
            return;
        }
    };

    if format != "mermaid" {
        eprintln!("❌ Unsupported format: {}. Only 'mermaid' is supported.", format);
        return;
    }

    use matrixcode_tui::workflow::export_mermaid;
    let mermaid_output = export_mermaid(&workflow_def, None);

    if let Some(output_path) = output {
        if let Err(e) = std::fs::write(&output_path, &mermaid_output) {
            eprintln!("❌ Failed to write output: {}", e);
        } else {
            println!("✓ Exported to: {}", output_path);
        }
    } else {
        println!("{}", mermaid_output);
    }
}

// Helper functions

fn create_provider(config: &Config, args: &CliArgs) -> Option<Arc<dyn matrixcode_core::providers::Provider>> {
    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok());

    if api_key.is_none() {
        eprintln!("Warning: No API key configured, AI tasks will not work");
        return None;
    }

    let model = args.model.clone();
    let provider_type = infer_provider_type(&model);
    let base_url = args.base_url.clone();

    match create_provider_with_headers(provider_type, api_key.unwrap(), model, Some(base_url), config.extra_headers.clone()) {
        Ok(p) => Some(Arc::from(p)),
        Err(e) => {
            eprintln!("Warning: Failed to create provider: {}", e);
            None
        }
    }
}

fn run_workflow_async(engine: WorkflowEngine, inputs: HashMap<String, serde_json::Value>) -> Result<matrixcode_core::workflow::WorkflowContext> {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(engine.run(inputs)))
    } else {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(engine.run(inputs))
    }
}

fn save_workflow_context(ctx: &matrixcode_core::workflow::WorkflowContext) {
    let project_path = std::env::current_dir().ok();
    let persistence = WorkflowPersistence::new(project_path.as_ref());
    if let Err(e) = persistence.save(ctx) {
        eprintln!("Warning: Failed to save workflow context: {}", e);
    }
}

fn source_label(source: &WorkflowSource) -> &'static str {
    match source {
        WorkflowSource::Project => "project",
        WorkflowSource::Global => "global",
    }
}

fn print_workflow_info(info: &matrixcode_core::workflow::WorkflowInfo) {
    let source = source_label(&info.source);
    println!("  {} - {} [{}]", info.id, info.name, source);
    if let Some(ref desc) = info.description {
        println!("    {}", desc.chars().take(60).collect::<String>());
    }
    if !info.required_inputs.is_empty() {
        println!("    Required: {}", info.required_inputs.join(", "));
    }
    println!("    File: {}", info.path.display());
    println!();
}

fn parse_status_filter(filter: &str) -> WorkflowStatus {
    match filter.to_lowercase().as_str() {
        "running" => WorkflowStatus::Running,
        "paused" => WorkflowStatus::Paused,
        "completed" => WorkflowStatus::Completed,
        "failed" => WorkflowStatus::Failed,
        "cancelled" => WorkflowStatus::Cancelled,
        _ => WorkflowStatus::Running,
    }
}