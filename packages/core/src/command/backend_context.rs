//! 后端命令执行上下文
//!
//! 包含命令执行所需的共享依赖。

use std::sync::{Arc, Mutex};

use crate::{AgentEvent, Config, SessionManager, agent::Agent, providers::Provider, skills::Skill};

/// 后端命令执行上下文
///
/// 包含命令执行所需的所有共享依赖。
/// 使用生命周期避免不必要的克隆。
pub struct BackendContext<'a> {
    /// 原始消息内容
    pub message: &'a str,
    /// 事件发送通道
    pub event_tx: &'a tokio::sync::mpsc::Sender<AgentEvent>,
    /// 项目路径
    pub project_path: Option<&'a std::path::PathBuf>,
    /// 可用技能列表
    pub skills: &'a [Skill],
    /// 配置
    pub config: &'a Config,
    /// 当前模型
    pub model: &'a str,
    /// 会话管理器
    pub session_mgr: &'a mut Option<SessionManager>,
    /// 记忆存储
    pub memory_storage: &'a mut Option<crate::memory::MemoryStorage>,
    /// Agent 实例
    pub agent: &'a mut Agent,
    /// Provider 实例
    pub provider: &'a dyn Provider,
    /// 文件监控句柄（可选）
    pub watcher_handle: Option<&'a Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>>,
    /// 取消令牌（可选）
    pub cancel_token: Option<&'a crate::cancel::CancellationToken>,
}