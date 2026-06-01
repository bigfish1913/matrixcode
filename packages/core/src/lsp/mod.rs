//! LSP (Language Server Protocol) Integration
//!
//! 提供语言服务器连接状态跟踪，用于 TUI 工具栏显示。
//!
//! # 架构概览
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  MatrixCode Agent                                           │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │  LSP Manager                                          │   │
//! │  │  ┌──────────────┐ ┌──────────────┐                   │   │
//! │  │  │ LspManager   │ │ LspServerInfo│                   │   │
//! │  │  │ (manager.rs) │ │ (types.rs)   │                   │   │
//! │  │  └──────────────┘ └──────────────┘                   │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//!                                          │
//! ┌───────────────────────────────────────────│─────────────────┐
//! │  Language Servers                         │                 │
//! │  ┌─────────────┐ ┌─────────────┐ ┌───────┴───┐            │
//! │  │ rust-analyzer│ │ typescript  │ │  python   │            │
//! │  │ LSP Server  │ │ LSP Server  │ │  LSP      │            │
//! │  └─────────────┘ └─────────────┘ └───────────┘            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 状态颜色
//!
//! - 灰色 (DarkGray)：未配置或未启动
//! - 绿色 (Green)：已连接，正常工作
//! - 红色 (Red)：连接错误
//!
//! # 使用示例
//!
//! ```ignore
//! use matrixcode_core::lsp::{LspManager, LspServerInfo, LspServerStatus};
//!
//! // 创建管理器
//! let manager = LspManager::new();
//!
//! // 添加服务器配置
//! manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
//!
//! // 获取状态列表（用于 TUI 显示）
//! let infos = manager.server_infos().await;
//! for info in infos {
//!     println!("{}: {}", info.name, info.status.label());
//! }
//! ```

pub mod client;
pub mod manager;
pub mod transport;
pub mod types;

// Re-export main types
pub use client::{LspClient, HoverResult, format_location, format_diagnostic, format_hover_result, path_to_uri};
pub use manager::{LspManager, find_lsp_config, load_lsp_config};
pub use transport::LspTransport;
pub use types::{
    LspConfig, LspServerConfig, LspServerInfo, LspServerStatus, default_lsp_config,
    default_python_config, default_rust_analyzer_config, default_typescript_config,
};

// ============================================================================
// Dynamic Injection Helpers (for Prompt System)
// ============================================================================

/// Check if LSP context should be injected into system prompt
///
/// Returns true if any server is connected and working properly.
/// This is used by the prompt system to conditionally inject LSP-related guidance.
pub fn should_inject_lsp_context(servers: &[LspServerInfo]) -> bool {
    servers.iter().any(|s| s.status.is_ok())
}

/// Get active (connected) LSP servers
///
/// Filters servers to only those with Connected status.
pub fn get_active_lsp_servers(servers: &[LspServerInfo]) -> Vec<&LspServerInfo> {
    servers.iter().filter(|s| s.status.is_ok()).collect()
}
