use ratatui::style::Color;

// Re-export ApproveMode from core
pub use matrixcode_core::ApproveMode;

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
    Asking, // Waiting for approval/ask response
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
            "search" | "grep" | "glob" | "ls" => Activity::Searching,
            "bash" => Activity::Running,
            "websearch" => Activity::WebSearch,
            "webfetch" => Activity::WebFetch,
            "task" | "task_create" | "task_get" | "task_list" | "task_stop" => {
                Activity::Tool("task".into())
            }
            "enter_plan_mode" | "exit_plan_mode" => Activity::Tool("plan".into()),
            "monitor" => Activity::Tool("monitor".into()),
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
    Tool {
        name: String,
        detail: Option<String>,
        is_error: bool,
        is_pending: bool, // Tool is currently executing (not yet completed)
    },
    System,
    Ask, // Approval/question requests - needs prominent display
}

impl Role {}

/// Ask option for interactive selection
#[derive(Debug, Clone, Default)]
pub struct AskOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub selected: bool,  // For multi-select: whether this option is checked
    pub is_submit: bool, // Whether this is a submit button option
    pub is_other: bool,  // Whether this is "Other" option that allows custom input
}

impl AskOption {
    /// Format description with prefix for display.
    pub fn format_description(&self) -> String {
        self.description
            .as_ref()
            .map(|d| format!(" - {}", d))
            .unwrap_or_default()
    }

    /// Create an "Other" option for custom input
    pub fn other_option() -> Self {
        Self {
            id: "other".to_string(),
            label: "其他 (自定义输入)".to_string(),
            description: Some("输入自定义内容".to_string()),
            selected: false,
            is_submit: false,
            is_other: true,
        }
    }
}

/// Submit mode for ask tool - determines how user confirms selection
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SubmitMode {
    #[default]
    Direct, // Enter directly submits (for few options)
    Option, // Submit as an option in the list (for medium options)
    Button, // Submit button area at bottom (for many options)
}

impl SubmitMode {
    /// Determine submit mode based on option count and multi_select flag.
    pub fn from_option_count(opt_count: usize, multi_select: bool) -> Self {
        if multi_select {
            if opt_count <= 3 {
                SubmitMode::Direct
            } else if opt_count <= 10 {
                SubmitMode::Option
            } else {
                SubmitMode::Button
            }
        } else {
            SubmitMode::Direct
        }
    }
}

/// Ask question with options - supports multiple questions
#[derive(Debug, Clone, Default)]
pub struct AskQuestion {
    pub id: String,
    pub question: String,
    pub options: Vec<AskOption>,
    pub multi_select: bool,
    pub selected_index: usize,
    pub submit_mode: SubmitMode,
    pub other_input: Option<String>, // Custom input when "Other" is selected
}

/// Message block
pub struct Message {
    pub role: Role,
    pub content: String,
    pub is_pending: bool, // true = 对接中（等待发送），false = 已发送
}
