//! Side Panel Component
//!
//! Collapsible panel with tabs for Tools, Skills, and Commands.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::app::AppState;

/// Panel tab type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelTab {
    /// Available tools
    Tools,
    /// Available skills
    Skills,
    /// Command help
    #[default]
    Commands,
}

impl PanelTab {
    /// Get tab title
    pub fn title(&self) -> &'static str {
        match self {
            PanelTab::Tools => "Tools",
            PanelTab::Skills => "Skills",
            PanelTab::Commands => "Commands",
        }
    }

    /// Get all tab titles
    pub fn titles() -> Vec<&'static str> {
        vec!["Tools", "Skills", "Commands"]
    }

    /// Get next tab
    pub fn next(&self) -> Self {
        match self {
            PanelTab::Tools => PanelTab::Skills,
            PanelTab::Skills => PanelTab::Commands,
            PanelTab::Commands => PanelTab::Tools,
        }
    }

    /// Get previous tab
    pub fn prev(&self) -> Self {
        match self {
            PanelTab::Tools => PanelTab::Commands,
            PanelTab::Skills => PanelTab::Tools,
            PanelTab::Commands => PanelTab::Skills,
        }
    }
}

/// Tool item for display
pub struct ToolItem {
    pub name: &'static str,
    pub description: &'static str,
}

/// Skill item for display
pub struct SkillItem {
    pub name: &'static str,
    pub description: &'static str,
}

/// Command item for display
pub struct CommandItem {
    pub command: &'static str,
    pub description: &'static str,
}

/// Side panel component
pub struct SidePanel {
    /// Current active tab
    active_tab: PanelTab,
    /// Scroll offset for list content
    scroll_offset: usize,
    /// List of available tools
    tools: Vec<ToolItem>,
    /// List of available skills
    skills: Vec<SkillItem>,
    /// List of commands
    commands: Vec<CommandItem>,
}

impl Default for SidePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SidePanel {
    /// Create new side panel
    pub fn new() -> Self {
        Self {
            active_tab: PanelTab::default(),
            scroll_offset: 0,
            tools: Self::get_default_tools(),
            skills: Self::get_default_skills(),
            commands: Self::get_default_commands(),
        }
    }

    /// Get default tools list
    fn get_default_tools() -> Vec<ToolItem> {
        vec![
            ToolItem { name: "Read", description: "Read file contents" },
            ToolItem { name: "Write", description: "Write content to file" },
            ToolItem { name: "Edit", description: "Edit file with diff" },
            ToolItem { name: "Glob", description: "Find files by pattern" },
            ToolItem { name: "Grep", description: "Search file contents" },
            ToolItem { name: "Bash", description: "Execute shell command" },
            ToolItem { name: "WebFetch", description: "Fetch URL content" },
            ToolItem { name: "WebSearch", description: "Search the web" },
            ToolItem { name: "TaskStop", description: "Stop running task" },
            ToolItem { name: "NotebookEdit", description: "Edit Jupyter notebook" },
        ]
    }

    /// Get default skills list
    fn get_default_skills() -> Vec<SkillItem> {
        vec![
            SkillItem { name: "om:start", description: "Start task execution" },
            SkillItem { name: "om:plan", description: "Generate task plan" },
            SkillItem { name: "om:debug", description: "Debug issues" },
            SkillItem { name: "om:check", description: "Check improvements" },
            SkillItem { name: "om:deploy", description: "Deploy application" },
            SkillItem { name: "om:status", description: "Show task status" },
            SkillItem { name: "om:brainstorm", description: "Brainstorm ideas" },
            SkillItem { name: "om:report", description: "Generate report" },
            SkillItem { name: "om:feature", description: "Feature workflow" },
            SkillItem { name: "om:resume", description: "Resume interrupted task" },
        ]
    }

    /// Get default commands list
    fn get_default_commands() -> Vec<CommandItem> {
        vec![
            CommandItem { command: "/help, /h, /?", description: "Show help" },
            CommandItem { command: "/exit, /quit, /q", description: "Exit application" },
            CommandItem { command: "/clear, /cls", description: "Clear screen" },
            CommandItem { command: "/model <name>", description: "Change model" },
            CommandItem { command: "/session list", description: "List sessions" },
            CommandItem { command: "/session save", description: "Save session" },
            CommandItem { command: "/session load <id>", description: "Load session" },
            CommandItem { command: "/session new", description: "New session" },
            CommandItem { command: "/session del <id>", description: "Delete session" },
        ]
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.scroll_offset = 0;
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.scroll_offset = 0;
    }

    /// Scroll up in content
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down in content
    pub fn scroll_down(&mut self, content_height: usize) {
        if self.scroll_offset < content_height.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Get active tab
    pub fn active_tab(&self) -> PanelTab {
        self.active_tab
    }

    /// Render the side panel
    pub fn render(&self, f: &mut Frame, state: &AppState, area: Rect) {
        // Create tabs
        let titles: Vec<Line> = PanelTab::titles()
            .iter()
            .map(|t| Line::from(*t))
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Panel ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            )
            .select(self.active_tab as usize)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        // Calculate areas
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(3), // Tabs
                ratatui::layout::Constraint::Min(0),    // Content
            ])
            .split(area);

        // Render tabs
        f.render_widget(tabs, chunks[0]);

        // Render content based on active tab
        let content = self.render_content(state, chunks[1]);
        f.render_widget(content, chunks[1]);
    }

    /// Render content for current tab
    fn render_content(&self, _state: &AppState, area: Rect) -> Paragraph<'static> {
        let lines = match self.active_tab {
            PanelTab::Tools => self.render_tools(),
            PanelTab::Skills => self.render_skills(),
            PanelTab::Commands => self.render_commands(),
        };

        // Apply scroll offset
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(self.scroll_offset)
            .take(area.height as usize)
            .collect();

        Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .style(Style::default().fg(Color::White)),
            )
    }

    /// Render tools list
    fn render_tools(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(Span::styled(
            " Available Tools:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for tool in &self.tools {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<12}", tool.name),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(" - ", Style::default().fg(Color::DarkGray)),
                Span::styled(tool.description, Style::default().fg(Color::Gray)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Use [Tab] to close panel",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    /// Render skills list
    fn render_skills(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(Span::styled(
            " Available Skills:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for skill in &self.skills {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<14}", skill.name),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(" - ", Style::default().fg(Color::DarkGray)),
                Span::styled(skill.description, Style::default().fg(Color::Gray)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Invoke with /skill_name",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    /// Render commands list
    fn render_commands(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(Span::styled(
            " Available Commands:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for cmd in &self.commands {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<20}", cmd.command),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(" - ", Style::default().fg(Color::DarkGray)),
                Span::styled(cmd.description, Style::default().fg(Color::Gray)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Type command and press Enter",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_tab_default() {
        let tab = PanelTab::default();
        assert_eq!(tab, PanelTab::Commands);
    }

    #[test]
    fn test_panel_tab_title() {
        assert_eq!(PanelTab::Tools.title(), "Tools");
        assert_eq!(PanelTab::Skills.title(), "Skills");
        assert_eq!(PanelTab::Commands.title(), "Commands");
    }

    #[test]
    fn test_panel_tab_titles() {
        let titles = PanelTab::titles();
        assert_eq!(titles, vec!["Tools", "Skills", "Commands"]);
    }

    #[test]
    fn test_panel_tab_next() {
        assert_eq!(PanelTab::Tools.next(), PanelTab::Skills);
        assert_eq!(PanelTab::Skills.next(), PanelTab::Commands);
        assert_eq!(PanelTab::Commands.next(), PanelTab::Tools);
    }

    #[test]
    fn test_panel_tab_prev() {
        assert_eq!(PanelTab::Tools.prev(), PanelTab::Commands);
        assert_eq!(PanelTab::Skills.prev(), PanelTab::Tools);
        assert_eq!(PanelTab::Commands.prev(), PanelTab::Skills);
    }

    #[test]
    fn test_panel_tab_cycle() {
        let tab = PanelTab::Tools;
        let next = tab.next();
        let next_next = next.next();
        let next_next_next = next_next.next();
        assert_eq!(next, PanelTab::Skills);
        assert_eq!(next_next, PanelTab::Commands);
        assert_eq!(next_next_next, PanelTab::Tools);
    }

    #[test]
    fn test_side_panel_new() {
        let panel = SidePanel::new();
        assert_eq!(panel.active_tab(), PanelTab::Commands);
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn test_side_panel_default() {
        let panel = SidePanel::default();
        assert_eq!(panel.active_tab(), PanelTab::Commands);
    }

    #[test]
    fn test_side_panel_next_tab() {
        let mut panel = SidePanel::new();
        panel.next_tab();
        assert_eq!(panel.active_tab(), PanelTab::Tools);
        panel.next_tab();
        assert_eq!(panel.active_tab(), PanelTab::Skills);
        panel.next_tab();
        assert_eq!(panel.active_tab(), PanelTab::Commands);
    }

    #[test]
    fn test_side_panel_prev_tab() {
        let mut panel = SidePanel::new();
        panel.prev_tab();
        assert_eq!(panel.active_tab(), PanelTab::Skills);
        panel.prev_tab();
        assert_eq!(panel.active_tab(), PanelTab::Tools);
        panel.prev_tab();
        assert_eq!(panel.active_tab(), PanelTab::Commands);
    }

    #[test]
    fn test_side_panel_scroll_up() {
        let mut panel = SidePanel::new();
        panel.scroll_offset = 5;
        panel.scroll_up();
        assert_eq!(panel.scroll_offset, 4);
    }

    #[test]
    fn test_side_panel_scroll_up_at_zero() {
        let mut panel = SidePanel::new();
        panel.scroll_offset = 0;
        panel.scroll_up();
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn test_side_panel_scroll_down() {
        let mut panel = SidePanel::new();
        panel.scroll_offset = 0;
        panel.scroll_down(10);
        assert_eq!(panel.scroll_offset, 1);
    }

    #[test]
    fn test_side_panel_scroll_down_respects_limit() {
        let mut panel = SidePanel::new();
        panel.scroll_offset = 8;
        panel.scroll_down(9);
        assert_eq!(panel.scroll_offset, 8); // Should not exceed content_height - 1
    }

    #[test]
    fn test_side_panel_scroll_reset_on_tab_change() {
        let mut panel = SidePanel::new();
        panel.scroll_offset = 10;
        panel.next_tab();
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn test_side_panel_tools_count() {
        let panel = SidePanel::new();
        assert_eq!(panel.tools.len(), 10);
    }

    #[test]
    fn test_side_panel_skills_count() {
        let panel = SidePanel::new();
        assert_eq!(panel.skills.len(), 10);
    }

    #[test]
    fn test_side_panel_commands_count() {
        let panel = SidePanel::new();
        assert_eq!(panel.commands.len(), 9);
    }

    #[test]
    fn test_render_tools() {
        let panel = SidePanel::new();
        let lines = panel.render_tools();
        assert!(!lines.is_empty());
        // First line should be header
        assert!(lines[0].spans.iter().any(|s| s.content.contains("Tools")));
    }

    #[test]
    fn test_render_skills() {
        let panel = SidePanel::new();
        let lines = panel.render_skills();
        assert!(!lines.is_empty());
        // First line should be header
        assert!(lines[0].spans.iter().any(|s| s.content.contains("Skills")));
    }

    #[test]
    fn test_render_commands() {
        let panel = SidePanel::new();
        let lines = panel.render_commands();
        assert!(!lines.is_empty());
        // First line should be header
        assert!(lines[0].spans.iter().any(|s| s.content.contains("Commands")));
    }

    #[test]
    fn test_tool_item() {
        let tool = ToolItem {
            name: "Test",
            description: "Test tool",
        };
        assert_eq!(tool.name, "Test");
        assert_eq!(tool.description, "Test tool");
    }

    #[test]
    fn test_skill_item() {
        let skill = SkillItem {
            name: "Test",
            description: "Test skill",
        };
        assert_eq!(skill.name, "Test");
        assert_eq!(skill.description, "Test skill");
    }

    #[test]
    fn test_command_item() {
        let cmd = CommandItem {
            command: "/test",
            description: "Test command",
        };
        assert_eq!(cmd.command, "/test");
        assert_eq!(cmd.description, "Test command");
    }
}