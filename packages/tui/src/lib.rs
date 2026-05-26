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
        terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
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
    execute!(
        std::io::stdout(),
        event::EnableMouseCapture,
        event::EnableBracketedPaste
    )?;
    let mut t = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    t.clear()?;
    Ok(t)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(
        std::io::stdout(),
        event::DisableMouseCapture,
        event::DisableBracketedPaste,
        Clear(ClearType::All),
        Show
    )?;
    Ok(())
}
