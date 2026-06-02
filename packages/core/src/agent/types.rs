//! Agent type definitions.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use tokio::sync::mpsc;

use crate::cancel::CancellationToken;
use crate::compress::CompressionConfig;
use crate::event::AgentEvent;
use crate::prompt::PromptProfile;
use crate::providers::Message;
#[cfg(test)]
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, StopReason, StreamEvent, Usage};
use crate::providers::Provider;
use crate::skills::Skill;
use crate::tools::{ReadHistoryTracker, Tool};
#[cfg(test)]
use async_trait::async_trait;

// Import new modular components
use super::core::{AgentConfig, AgentState};
use super::context::AgentContext;
use super::session::SessionManager;

// Re-export for backward compatibility
pub use super::core::MAX_ITERATIONS;

/// **MAX_ITERATIONS Documentation**:
///
/// **Why 200 iterations?**
/// - Sufficient for most common tasks (file edits, code review, simple builds)
/// - Prevents infinite loops and runaway operations
/// - Balances task completion with resource efficiency
///
/// **What happens when limit is reached?**
/// - Agent stops execution gracefully
/// - User receives detailed warning message explaining:
///   - Task status (may not be complete)
///   - Reason for stopping (iteration limit)
///   - Next steps (continue, break down task, or resume)
///
/// **Future improvements**:
/// - Dynamic adjustment based on task complexity
/// - User-configurable limits in config file
/// - Auto-resume with state preservation
/// - Progress indicators showing iteration count
///
/// **Examples**:
/// - Simple task (edit file): ~5-10 iterations
/// - Medium task (refactor module): ~15-30 iterations
/// - Complex task (build system): ~40-50 iterations (may hit limit)
///
/// Full Agent with event output
///
/// # Architecture (Refactored)
/// The Agent now uses modular components:
/// - `config`: Configuration constants (max iterations, retries, etc.)
/// - `state`: Mutable state (messages, tokens, todos)
/// - `context`: Context management (system prompt, skills, memory)
/// - `session`: Session lifecycle (events, cancellation)
pub struct Agent {
    // === Core Components (New) ===
    /// Configuration constants
    pub(crate) config: AgentConfig,
    /// Mutable state management (kept for future use)
    pub(crate) state: AgentState,
    /// Context management
    pub(crate) context: AgentContext,
    /// Session lifecycle
    pub(crate) session: SessionManager,

    // === Provider & Tools ===
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,

    // === Event Channel ===
    /// Event sender (kept for frequent access)
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,

    // === Approval & Permissions ===
    pub(crate) approve_mode: Arc<AtomicU8>,

    // === Legacy Fields (will be migrated to components in future phases) ===
    /// Messages history (TODO: migrate to AgentState in Phase 4)
    pub(crate) messages: Vec<Message>,
    /// System prompt (TODO: migrate to AgentContext accessor)
    pub(crate) system_prompt: String,
    /// Compression config (TODO: use AgentConfig.compression_config)
    pub(crate) compression_config: CompressionConfig,
    /// Cancellation token (TODO: use SessionManager.cancel_token)
    pub(crate) cancel_token: Option<CancellationToken>,
    /// Token tracking (TODO: migrate to AgentState in Phase 4)
    pub(crate) total_input_tokens: std::sync::atomic::AtomicU64,
    pub(crate) total_output_tokens: std::sync::atomic::AtomicU64,
    pub(crate) last_input_tokens: std::sync::atomic::AtomicU64,
    /// Todo reminders (TODO: migrate to AgentState in Phase 4)
    pub(crate) todo_reminder_count: std::collections::HashMap<String, usize>,
    /// Pending inputs (TODO: migrate to AgentState in Phase 4)
    pub(crate) pending_inputs: Vec<String>,
    /// Ask response channel (TODO: use SessionManager.ask_rx)
    pub(crate) ask_rx: Option<mpsc::Receiver<String>>,

    // === Tool Tracking ===
    /// Tool ids whose full input was already sent to the UI during streaming.
    pub(crate) previewed_tool_inputs: HashSet<String>,
    /// Read history tracker: tracks files that have been read in this session.
    /// Used to enforce "read before edit/write" rule.
    pub(crate) read_history: ReadHistoryTracker,

    // === Proxy Tools ===
    /// 代理工具定义列表（发送给 LLM）
    pub(crate) proxy_tool_defs: Vec<crate::tools::toolproxy::ProxyToolDef>,
    /// 代理工具执行器
    pub(crate) proxy_executor: Option<Arc<dyn crate::tools::toolproxy::ProxyToolExecutor>>,

    // === External Registries ===
    /// MCP 工具注册表（动态管理）
    pub(crate) mcp_registry: Option<Arc<tokio::sync::RwLock<crate::mcp::McpToolRegistry>>>,
    /// LSP 客户端注册表（用于 LSP 工具）
    #[allow(dead_code)] // TODO: 实现 LSP 工具集成后移除
    pub(crate) lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,

    // === Input Channels ===
    /// 实时追加消息接收器（用于在处理过程中接收新消息）
    pub(crate) pending_input_rx: Option<mpsc::Receiver<String>>,
}

/// Agent builder
///
/// Simplified builder that constructs Agent using modular components.
pub struct AgentBuilder {
    // === Provider & Tools ===
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,

    // === Config (AgentConfig) ===
    pub(crate) max_tokens: u32,
    pub(crate) context_size_override: Option<u32>,
    pub(crate) think: bool,
    pub(crate) compression_config: CompressionConfig,

    // === Context (AgentContext) ===
    pub(crate) profile: PromptProfile,
    pub(crate) skills: Vec<Skill>,
    pub(crate) project_overview: Option<String>,
    pub(crate) memory_summary: Option<String>,
    pub(crate) project_path: Option<PathBuf>,

    // === Session (SessionManager) ===
    pub(crate) event_tx: Option<mpsc::Sender<AgentEvent>>,
    pub(crate) pending_input_rx: Option<mpsc::Receiver<String>>,

    // === Approval ===
    pub(crate) approve_mode: crate::approval::ApproveMode,

    // === Proxy Tools ===
    pub(crate) proxy_tool_defs: Vec<crate::tools::toolproxy::ProxyToolDef>,
    pub(crate) proxy_executor: Option<Arc<dyn crate::tools::toolproxy::ProxyToolExecutor>>,

    // === External Registries ===
    pub(crate) mcp_registry: Option<Arc<tokio::sync::RwLock<crate::mcp::McpToolRegistry>>>,
    pub(crate) lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,
}

// 注意：AgentBuilder 必须通过 AgentBuilder::new(provider) 创建
// Default 实现仅供内部测试使用，不应在生产环境使用
#[cfg(test)]
impl Default for AgentBuilder {
    fn default() -> Self {
        // 测试��境下使用 Mock Provider
        Self::new(Box::new(MockTestProvider))
    }
}

#[cfg(test)]
struct MockTestProvider;

#[cfg(test)]
#[async_trait]
impl Provider for MockTestProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        Ok(ChatResponse {
            content: vec![ContentBlock::Text {
                text: "mock".to_string(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        Ok(rx)
    }

    fn model_name(&self) -> &str {
        "mock"
    }

    fn context_size(&self) -> Option<u32> {
        Some(200_000)
    }

    fn clone_box(&self) -> Box<dyn Provider> {
        Box::new(MockTestProvider)
    }

    fn clone_arc(&self) -> std::sync::Arc<dyn Provider> {
        std::sync::Arc::new(MockTestProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentBuilder;

    #[test]
    fn test_effective_context_size_prefers_override() {
        let agent = AgentBuilder::new(Box::new(MockTestProvider))
            .context_size(Some(1_000_000))
            .build();

        assert_eq!(agent.effective_context_size(), Some(1_000_000));
    }

    #[test]
    fn test_effective_context_size_falls_back_to_provider() {
        let agent = AgentBuilder::new(Box::new(MockTestProvider)).build();

        assert_eq!(agent.effective_context_size(), Some(200_000));
    }
}
