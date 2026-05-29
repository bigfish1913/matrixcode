//! 命令注册表
//!
//! 管理所有已注册命令的查找和分发。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use super::backend_context::BackendContext;
use super::command_trait::Command;

/// 全局命令注册表
static REGISTRY: LazyLock<Mutex<CommandRegistry>> = LazyLock::new(|| {
    let mut registry = CommandRegistry::new();
    super::handlers::register_commands(&mut registry);
    Mutex::new(registry)
});

/// 获取全局注册表引用
pub fn get_registry() -> &'static Mutex<CommandRegistry> {
    &REGISTRY
}

/// 命令注册表
///
/// 存储所有已注册命令，支持按名称查找和分发。
pub struct CommandRegistry {
    commands: Vec<Arc<dyn Command>>,
    name_index: HashMap<String, usize>,
}

impl CommandRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            name_index: HashMap::new(),
        }
    }

    /// 注册命令
    pub fn register(&mut self, command: Arc<dyn Command>) {
        let idx = self.commands.len();
        self.name_index.insert(command.name().to_string(), idx);

        // 注册别名
        for alias in command.aliases() {
            self.name_index.insert(alias.to_string(), idx);
        }

        self.commands.push(command);
    }

    /// 查找匹配的命令
    pub fn find(&self, msg: &str) -> Option<Arc<dyn Command>> {
        // 尝试精确匹配或前缀匹配
        for cmd in &self.commands {
            if cmd.matches(msg) {
                return Some(cmd.clone());
            }
        }
        None
    }

    /// 分发命令执行
    ///
    /// 返回值：
    /// - `Some(true)`：命令执行完成，消息应转发给 agent
    /// - `Some(false)`：命令已处理，不转发
    /// - `None`：未找到匹配命令
    pub async fn dispatch(&self, msg: &str, ctx: &mut BackendContext<'_>) -> Option<bool> {
        let cmd = self.find(msg)?;
        Some(cmd.execute(ctx).await)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
