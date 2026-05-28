//! /help command

use crate::commands::{Command, CommandContext};

pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show this help")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.push_system(concat!(
            "📖 Commands:\n",
            "  /help     - Show this help\n",
            "  /exit     - Exit MatrixCode\n",
            "  /clear    - Clear messages\n",
            "  /history  - Show session history\n",
            "  /mode     - Change approve mode (ask/auto/strict)\n",
            "  /model    - Show/change model\n",
            "  /config   - Show current configuration\n",
            "  /tools    - List available tools with descriptions\n",
            "  /system   - Show system prompt and all details\n",
            "  /compact  - Compress context\n",
            "  /retry    - Retry last queued message\n",
            "  /new      - Start new session\n",
            "  /init     - Initialize/reset project\n",
            "  /skills   - List loaded skills\n",
            "  /memory   - View/manage memories\n",
            "  /overview - View project overview\n",
            "  /save     - Save current session\n",
            "  /sessions - List saved sessions\n",
            "  /load <id>- Load a session\n",
            "  /debug    - Toggle debug mode\n",
            "  /loop     - Start/stop loop task\n",
            "  /cron     - Manage scheduled tasks\n",
            "\n",
            "⌨️ Shortcuts:\n",
            "  Enter=send │ Shift+Enter=newline │ Ctrl+V=paste\n",
            "  ↑↓=history │ PgUp/PgDn=scroll │ Home/End=top/bot\n",
            "  Alt+M=mode │ Alt+T=collapse │ Esc=interrupt │ Ctrl+D=exit"
        ).into());
        ctx.auto_scroll();
    }
}