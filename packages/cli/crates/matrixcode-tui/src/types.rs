use ratatui::style::Color;

/// Activity state
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Activity {
    #[default]
    Idle,
    Thinking,
    Reading,
    Writing,
    Editing,
    Searching,
    Running,
    WebSearch,
    WebFetch,
    Tool(String),
    Asking,  // Waiting for approval/ask response
}

impl Activity {
    pub fn label(&self) -> String {
        match self {
            Activity::Idle => "Ready".into(),
            Activity::Thinking => "Thinking".into(),
            Activity::Reading => "read".into(),
            Activity::Writing => "write".into(),
            Activity::Editing => "edit".into(),
            Activity::Searching => "search".into(),
            Activity::Running => "bash".into(),
            Activity::WebSearch => "websearch".into(),
            Activity::WebFetch => "webfetch".into(),
            Activity::Tool(name) => name.clone(),
            Activity::Asking => "AWAITING".into(),
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Activity::Idle => Color::Green,
            Activity::Thinking => Color::Magenta,
            Activity::Reading | Activity::Searching => Color::Cyan,
            Activity::Writing | Activity::Editing => Color::Yellow,
            Activity::Running => Color::Red,
            Activity::WebSearch | Activity::WebFetch => Color::Blue,
            Activity::Tool(_) => Color::Cyan,
            Activity::Asking => Color::Red,
        }
    }

    pub fn from_tool(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "read" => Activity::Reading,
            "write" => Activity::Writing,
            "edit" | "multi_edit" => Activity::Editing,
            "search" | "glob" | "ls" => Activity::Searching,
            "bash" => Activity::Running,
            "websearch" => Activity::WebSearch,
            "webfetch" => Activity::WebFetch,
            other => Activity::Tool(other.to_string()),
        }
    }
}

/// Message role
#[derive(PartialEq)]
pub enum Role {
    User,
    Assistant,
    Thinking,
    Tool { name: String, is_error: bool },
    System,
}

impl Role {
    pub fn icon(&self) -> &'static str {
        match self {
            Role::User => "👤",
            Role::Assistant => "🤖",
            Role::Thinking => "💭",
            Role::Tool { is_error, .. } => if *is_error { "❌" } else { "✅" },
            Role::System => "⚠️",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Role::User => "You".into(),
            Role::Assistant => "Assistant".into(),
            Role::Thinking => "Thinking".into(),
            Role::Tool { name, .. } => name.clone(),
            Role::System => "System".into(),
        }
    }

    #[allow(dead_code)]
    pub fn color(&self) -> Color {
        match self {
            Role::User => Color::Green,
            Role::Assistant => Color::Blue,
            Role::Thinking => Color::Magenta,
            Role::Tool { is_error, .. } => if *is_error { Color::Red } else { Color::Cyan },
            Role::System => Color::Yellow,
        }
    }
}

/// Message block
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Approval mode for tool execution
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ApproveMode {
    #[default]
    Ask,
    Auto,
    Strict,
}

impl ApproveMode {
    pub fn label(&self) -> &'static str {
        match self {
            ApproveMode::Ask => "ask",
            ApproveMode::Auto => "auto",
            ApproveMode::Strict => "strict",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ApproveMode::Ask => ApproveMode::Auto,
            ApproveMode::Auto => ApproveMode::Strict,
            ApproveMode::Strict => ApproveMode::Ask,
        }
    }

    /// Convert to u8 for atomic storage (matches matrixcode_core::approval::ApproveMode).
    pub fn to_u8(&self) -> u8 {
        match self {
            ApproveMode::Auto => 0,
            ApproveMode::Ask => 1,
            ApproveMode::Strict => 2,
        }
    }
}