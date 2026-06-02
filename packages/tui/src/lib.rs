mod app;
mod commands;
mod draw;
mod events;
pub mod image_search;
pub mod image_utils;
mod input;
mod markdown;
mod types;
mod utils;
pub mod workflow;

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        cursor::Show,
        event, execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
};
use std::io::Stdout;

pub use app::TuiApp;
pub use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};
// Re-export crossterm for CLI use
pub use ratatui::crossterm;

pub(crate) const ANIM_MS: u64 = 80;
pub(crate) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Border/padding width for message rendering
pub(crate) const BORDER_PADDING: usize = 4;

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    // 使用 alternate screen（干净的 TUI 界面）
    // 启用鼠标捕获用于滚轮滚动 TUI 内部消息
    // 用户可以用 Shift+鼠标选择文本复制，或 Shift+滚轮滚动终端缓冲区
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        event::EnableMouseCapture,
        event::EnableBracketedPaste
    )?;
    let t = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    Ok(t)
}

pub fn restore_terminal() -> Result<()> {
    // 先离开 alternate screen，回到主屏幕
    execute!(
        std::io::stdout(),
        LeaveAlternateScreen,
        event::DisableMouseCapture,
        event::DisableBracketedPaste,
        Show
    )?;
    disable_raw_mode()?;
    Ok(())
}
