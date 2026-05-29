//! 命令 trait 定义
//!
//! 每个命令实现此 trait 并注册到 CommandRegistry。

use std::future::Future;
use std::pin::Pin;

use super::backend_context::BackendContext;

/// 命令 trait，用于实现后端命令
///
/// 每个命令需要实现此 trait 并注册到 CommandRegistry。
pub trait Command: Send + Sync {
    /// 命令名称（不含前导 /）
    fn name(&self) -> &'static str;

    /// 命令别名（不含前导 /）
    fn aliases(&self) -> &[&'static str] {
        &[]
    }

    /// 命令帮助文本
    fn help(&self) -> Option<&'static str> {
        None
    }

    /// 检查命令是否匹配给定消息
    ///
    /// 默认匹配：
    /// - 精确匹配：`/name`
    /// - 带参数：`/name ...`
    /// - 别名：`/alias` 或 `/alias ...`
    fn matches(&self, msg: &str) -> bool {
        let name = self.name();
        if msg == format!("/{}", name) || msg.starts_with(&format!("/{} ", name)) {
            return true;
        }

        for alias in self.aliases() {
            if msg == format!("/{}", alias) || msg.starts_with(&format!("/{} ", alias)) {
                return true;
            }
        }

        false
    }

    /// 异步执行命令
    ///
    /// 返回值：
    /// - `true`：消息继续转发给 agent
    /// - `false`：命令已处理完毕，不再转发
    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}
