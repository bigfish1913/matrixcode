//! Agent type definitions.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64};
use tokio::sync::mpsc;

use crate::cancel::CancellationToken;
use crate::compress::CompressionConfig;
use crate::event::AgentEvent;
use crate::prompt::PromptProfile;
#[cfg(test)]
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, StopReason, StreamEvent, Usage};
use crate::providers::{Message, Provider};
use crate::skills::Skill;
use crate::tools::Tool;
#[cfg(test)]
use async_trait::async_trait;

pub(crate) const MAX_ITERATIONS: usize = 200;

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
pub struct Agent {
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) messages: Vec<Message>,
    pub(crate) system_prompt: String,
    pub(crate) max_tokens: u32,
    pub(crate) context_size_override: Option<u32>,
    pub(crate) think: bool,
    pub(crate) approve_mode: Arc<AtomicU8>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    pub(crate) skills: Vec<Skill>,
    pub(crate) profile: PromptProfile,
    pub(crate) project_overview: Option<String>,
    pub(crate) memory_summary: Option<String>,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) total_input_tokens: AtomicU64,
    pub(crate) total_output_tokens: AtomicU64,
    pub(crate) last_input_tokens: AtomicU64,
    pub(crate) cancel_token: Option<CancellationToken>,
    pub(crate) compression_config: CompressionConfig,
    pub(crate) ask_rx: Option<mpsc::Receiver<String>>,
    /// 代理工具定义列表（发送给 LLM）
    pub(crate) proxy_tool_defs: Vec<crate::tools::toolproxy::ProxyToolDef>,
    /// 代理工具执行器
    pub(crate) proxy_executor: Option<Arc<dyn crate::tools::toolproxy::ProxyToolExecutor>>,
    /// MCP 工具注册表（动态管理）
    pub(crate) mcp_registry: Option<Arc<tokio::sync::RwLock<crate::mcp::McpToolRegistry>>>,
    /// 实时追加消息接收器（用于在处理过程中接收新消息）
    pub(crate) pending_input_rx: Option<mpsc::Receiver<String>>,
    /// 缓存的追加消息（在当前轮完成后处理）
    pub(crate) pending_inputs: Vec<String>,
    /// Tool ids whose full input was already sent to the UI during streaming.
    pub(crate) previewed_tool_inputs: HashSet<String>,
    /// Todo reminder count: maps todo content hash to reminder count.
    /// Used to prevent infinite loops when model doesn't update todo status.
    pub(crate) todo_reminder_count: HashMap<String, usize>,
}

/// Agent builder
pub struct AgentBuilder {
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) model_name: String,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) system_prompt: String,
    pub(crate) max_tokens: u32,
    pub(crate) context_size_override: Option<u32>,
    pub(crate) think: bool,
    pub(crate) approve_mode: crate::approval::ApproveMode,
    pub(crate) event_tx: Option<mpsc::Sender<AgentEvent>>,
    pub(crate) skills: Vec<Skill>,
    pub(crate) profile: PromptProfile,
    pub(crate) project_overview: Option<String>,
    pub(crate) memory_summary: Option<String>,
    pub(crate) project_path: Option<PathBuf>,
    /// 代理工具定义列表
    pub(crate) proxy_tool_defs: Vec<crate::tools::toolproxy::ProxyToolDef>,
    /// 代理工具执行器
    pub(crate) proxy_executor: Option<Arc<dyn crate::tools::toolproxy::ProxyToolExecutor>>,
    /// MCP 工具注册表（动态管理）
    pub(crate) mcp_registry: Option<Arc<tokio::sync::RwLock<crate::mcp::McpToolRegistry>>>,
    /// 实时追加消息接收器
    pub(crate) pending_input_rx: Option<mpsc::Receiver<String>>,
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
