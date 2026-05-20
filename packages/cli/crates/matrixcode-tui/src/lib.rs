mod types;
mod utils;
mod markdown;
mod app;
mod draw;
mod input;
mod commands;
mod events;

use anyhow::Result;
use std::io::Stdout;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event,
        terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
        execute, cursor::Show,
    },
    Terminal,
};

pub use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};
pub use app::TuiApp;

pub(crate) const ANIM_MS: u64 = 80;
pub(crate) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), event::EnableMouseCapture)?;
    let mut t = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    t.clear()?;
    Ok(t)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), event::DisableMouseCapture, Clear(ClearType::All), Show)?;
    Ok(())
}
