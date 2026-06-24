//! 终端模式模块
//!
//! 提供清晰的模块化架构：
//! - `setup` - 初始化（配置、API、运行时、通道）
//! - `session` - 会话管理（加载、恢复、保存、列表）
//! - `watcher` - CodeGraph 监控管理
//! - `mcp_handler` - MCP 服务器生命周期
//! - `lsp_handler` - LSP 服务器生命周期
//! - `memory_handler` - 记忆检索、反馈、提取
//! - `commands` - 后端命令处理器（策略模式）
//! - `agent` - Agent 任务执行

pub mod setup;
pub mod session;
pub mod watcher;
pub mod mcp_handler;
pub mod lsp_handler;
pub mod memory_handler;
pub mod commands;
pub mod agent;

// Re-export main functions for main.rs
pub use setup::{run_terminal_mode, interactive_resume, list_sessions};