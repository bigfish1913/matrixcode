//! /shortcuts command - show all keyboard shortcuts

use crate::commands::{Command, CommandContext};

pub struct ShortcutsCommand;

impl Command for ShortcutsCommand {
    fn name(&self) -> &'static str {
        "shortcuts"
    }

    fn aliases(&self) -> &[&'static str] {
        &["keys", "快捷键"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("显示所有快捷键")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.push_system(
            concat!(
                "⌨️ 快捷键完整列表\n",
                "\n",
                "📝 输入编辑:\n",
                "  [Enter]        发送消息\n",
                "  [Shift+Enter]  换行 (多行输入)\n",
                "  [Ctrl+V]       粘贴剪贴板内容\n",
                "  [Ctrl+D]       退出程序\n",
                "  [Esc]          清空输入/中断操作/取消\n",
                "\n",
                "📜 历史与导航:\n",
                "  [↑]            浏览历史记录 (上一条)\n",
                "  [↓]            浏览历史记录 (下一条)\n",
                "  [Home]         光标移到开头/滚动到顶部\n",
                "  [End]          光标移到结尾/滚动到底部\n",
                "  [←/→]          光标左右移动\n",
                "\n",
                "📄 滚动浏览:\n",
                "  [PgUp]         向上滚动 10 行\n",
                "  [PgDn]         向下滚动 10 行\n",
                "  [Alt+↑]        向上滚动 1 行\n",
                "  [Alt+↓]        向下滚动 1 行\n",
                "  鼠标滚轮        滚动 3 行\n",
                "\n",
                "⚙️ 功能切换:\n",
                "  [Alt+M]        切换批准模式 (Ask/Auto/Strict)\n",
                "  [Shift+Tab]    切换批准模式\n",
                "  [Alt+T]        切换思考内容/输入折叠\n",
                "  [Alt+W]        切换工作流面板\n",
                "\n",
                "🔧 Ask/问答模式:\n",
                "  [↑↓]           选择选项\n",
                "  [Space]        切换选项 (多选模式)\n",
                "  [Enter]        确认选择\n",
                "  [Tab]          切换问题 (多问题模式)\n",
                "\n",
                "🐛 调试模式 (debug_mode 开启时):\n",
                "  [Shift+D]      切换调试面板\n",
                "  [Shift+C]      清除调试日志\n",
                "  [↑↓]           滚动调试日志\n",
                "\n",
                "⚡ 操作控制:\n",
                "  [Esc]          中断当前操作\n",
                "  [Ctrl+C]       强制中断\n",
                "  [Shift+Esc]    移除队列首条消息\n",
                "\n",
                "💡 提示:\n",
                "  多行粘贴后需按两次 [Enter] 发送\n",
                "  滚动时新消息会显示通知"
            )
            .into(),
        );
        ctx.auto_scroll();
    }
}