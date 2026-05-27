//! CLI types and argument definitions

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::commands::WorkflowCommands;
use crate::constants::DEFAULT_MAX_TOKENS;

/// CLI arguments
#[derive(Parser)]
#[command(name = "matrixcode")]
#[command(about = "AI Code Agent with multi-model support")]
#[command(version)]
pub struct Cli {
    /// Run mode
    #[arg(short, long, default_value = "terminal")]
    pub mode: String,

    /// Continue last session
    #[arg(short, long)]
    pub continue_session: bool,

    /// Resume session (interactive selection)
    #[arg(short = 'r', long)]
    pub resume: bool,

    /// Resume specific session by ID (non-interactive)
    #[arg(long)]
    pub resume_id: Option<String>,

    /// List sessions
    #[arg(long)]
    pub list_sessions: bool,

    /// Extra skills directory
    #[arg(long)]
    pub skills_dir: Option<PathBuf>,

    /// Think mode (optional, uses config default if not specified)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub think: Option<bool>,

    /// Max tokens
    #[arg(long, default_value_t = DEFAULT_MAX_TOKENS)]
    pub max_tokens: u32,

    /// MCP server to connect (format: name=command,args)
    /// Example: --mcp playwright=npx,-y,@playwright/mcp@latest
    /// Multiple servers: --mcp playwright=... --mcp filesystem=...
    #[arg(long, value_name = "SPEC")]
    pub mcp: Vec<String>,

    /// Disable loading MCP servers from config
    #[arg(long)]
    pub no_mcp: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start chat session
    Chat {
        /// Input content
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Quick action
    QuickAction {
        /// Action type
        #[arg(short, long)]
        action: String,

        /// Target file
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Create new session
    NewSession,

    /// Show session history
    History,

    /// Show status
    Status,

    /// Workflow management commands
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
}