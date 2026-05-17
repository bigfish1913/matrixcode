//! UI Components
//!
//! This module contains all the UI components for the TUI.

mod status_bar;
mod output_area;
mod input_box;
mod side_panel;

pub use status_bar::StatusBar;
pub use output_area::OutputArea;
pub use input_box::InputBox;
pub use side_panel::{SidePanel, PanelTab, ToolItem, SkillItem, CommandItem};