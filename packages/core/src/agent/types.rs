//! Agent type definitions.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64};
use tokio::sync::mpsc;

use crate::cancel::CancellationToken;
use crate::compress::CompressionConfig;
use crate::event::AgentEvent;
use crate::prompt::PromptProfile;
use crate::providers::{Message, Provider};
use crate::skills::Skill;
use crate::tools::Tool;

pub(crate) const MAX_ITERATIONS: usize = 50;

/// Full Agent with event output
#[allow(dead_code)]
pub struct Agent {
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) messages: Vec<Message>,
    pub(crate) system_prompt: String,
    pub(crate) max_tokens: u32,
    pub(crate) think: bool,
    pub(crate) approve_mode: Arc<AtomicU8>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    pub(crate) skills: Vec<Skill>,
    pub(crate) profile: PromptProfile,
    pub(crate) project_overview: Option<String>,
    pub(crate) memory_summary: Option<String>,
    pub(crate) total_input_tokens: AtomicU64,
    pub(crate) total_output_tokens: AtomicU64,
    pub(crate) last_input_tokens: AtomicU64,
    pub(crate) cancel_token: Option<CancellationToken>,
    pub(crate) compression_config: CompressionConfig,
    pub(crate) ask_rx: Option<mpsc::Receiver<String>>,
}

/// Agent builder
pub struct AgentBuilder {
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) system_prompt: String,
    pub(crate) max_tokens: u32,
    pub(crate) think: bool,
    pub(crate) approve_mode: crate::approval::ApproveMode,
    pub(crate) event_tx: Option<mpsc::Sender<AgentEvent>>,
    pub(crate) skills: Vec<Skill>,
    pub(crate) profile: PromptProfile,
    pub(crate) project_overview: Option<String>,
    pub(crate) memory_summary: Option<String>,
}
