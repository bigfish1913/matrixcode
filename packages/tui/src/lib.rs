mod app;
mod commands;
mod draw;
mod events;
mod input;
mod markdown;
mod types;
mod utils;
pub mod image_utils;
pub mod image_search;
pub mod workflow;

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        cursor::Show,
        event, execute,
        terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    },
};
use std::io::Stdout;

pub use app::TuiApp;
pub use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};
// Re-export crossterm for CLI use
pub use ratatui::crossterm;

pub(crate) const ANIM_MS: u64 = 80;
pub(crate) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    // Clear screen at startup to remove previous terminal content
    execute!(
        std::io::stdout(),
        Clear(ClearType::All),
        event::EnableMouseCapture,
        event::EnableBracketedPaste
    )?;
    let t = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    Ok(t)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    // Clear screen at exit to remove app content
    execute!(
        std::io::stdout(),
        Clear(ClearType::All),
        event::DisableMouseCapture,
        event::DisableBracketedPaste,
        Show
    )?;
    Ok(())
}
