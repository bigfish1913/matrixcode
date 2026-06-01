//! /help command

use crate::commands::{Command, CommandContext};

pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn aliases(&self) -> &[&'static str] {
        &["?", "帮助"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("显示帮助信息")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.push_system(
            concat!(
                "📖 命令列表:\n",
                "  /help      - 显示帮助信息\n",
                "  /shortcuts - 显示完整快捷键列表\n",
                "  /exit      - 退出程序\n",
                "  /clear     - 清空消息\n",
                "  /history   - 显示会话历史\n",
                "  /mode      - 切换批准模式\n",
                "  /model     - 显示/切换模型\n",
                "  /config    - 显示配置信息\n",
                "  /tools     - 列出可用工具\n",
                "  /skills    - 列出已加载技能\n",
                "  /memory    - 查看/管理记忆\n",
                "  /compact   - 压缩上下文\n",
                "  /retry     - 重试最后消息\n",
                "  /new       - 新建会话\n",
                "  /save      - 保存会话\n",
                "  /sessions  - 列出已保存会话\n",
                "  /load <id> - 加载会话\n",
                "  /init      - 初始化项目\n",
                "  /debug     - 切换调试模式\n",
                "\n",
                "⌨️ 常用快捷键:\n",
                "  [Enter] 发送 │ [Shift+Enter] 换行 │ [Ctrl+V] 粘贴\n",
                "  [↑↓] 历史 │ [PgUp/PgDn] 滚动 │ [Esc] 中断/取消\n",
                "  [Alt+M] 模式 │ [Alt+T] 折叠 │ [Alt+W] 工作流\n",
                "\n",
                "💡 输入 /shortcuts 查看完整快捷键列表"
            )
            .into(),
        );
        ctx.auto_scroll();
    }
}
