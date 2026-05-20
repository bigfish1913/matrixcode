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

pub(crate) const ANIM_MS: u64 = 100;

// Matrix-style spinner: square logo with rotating block
// Uses Unicode quadrant blocks (▘▝▖▗) that each fill one quarter of a cell
// Animation: a single quarter block rotates around the 4 corners, then fills
// ▘ = top-left, ▝ = top-right, ▖ = bottom-left, ▗ = bottom-right
pub(crate) const MATRIX_SPINNER: [&str; 12] = [
    "▘ ",    // Frame 1: top-left
    " ▝",    // Frame 2: top-right
    "▖ ",    // Frame 3: bottom-left
    " ▗",    // Frame 4: bottom-right
    "▘▝",    // Frame 5: top row
    "▖▗",    // Frame 6: bottom row
    "▘▖",    // Frame 7: left column
    "▝▗",    // Frame 8: right column
    "▙▚",    // Frame 9: major blocks (3/4 left + half right)
    "██",    // Frame 10: full solid
    "▓▓",    // Frame 11: dense shade
    "░░",    // Frame 12: sparse shade (reset)
];

// Logo colors for animated spinner (cycles through: green, blue, orange)
pub(crate) const LOGO_COLORS: [ratatui::style::Color; 12] = [
    ratatui::style::Color::LightGreen,   // Frame 1: top-left - green
    ratatui::style::Color::LightBlue,    // Frame 2: top-right - blue
    ratatui::style::Color::LightYellow,  // Frame 3: bottom-left - orange
    ratatui::style::Color::LightGreen,   // Frame 4: bottom-right - green
    ratatui::style::Color::LightBlue,    // Frame 5: top row - blue
    ratatui::style::Color::LightYellow,  // Frame 6: bottom row - orange
    ratatui::style::Color::LightGreen,   // Frame 7: left column - green
    ratatui::style::Color::LightBlue,    // Frame 8: right column - blue
    ratatui::style::Color::LightYellow,  // Frame 9: mixed - orange
    ratatui::style::Color::LightGreen,   // Frame 10: full solid - green
    ratatui::style::Color::LightBlue,    // Frame 11: dense shade - blue
    ratatui::style::Color::LightYellow,  // Frame 12: sparse shade - orange
];

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
