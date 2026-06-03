//! Session Start Hooks - Dynamic prompt injection
//!
//! This module provides hooks that inject content at session start:
//! - SessionStart hook: Inject skill rules, red flags before user message
//! - TodoReminder: Remind pending tasks from TodoWrite
//! - DiagnosticsInjection: Real-time LSP diagnostics
//!
//! ## Injection Order
//!
//! ```
//! Session Start:
//! ├── Core system prompt (static)
//! ├── Environment info (startup)
//! ├── SessionStart hook content ← This module
//! ├── Deferred tools (MCP)
//! ├── Todo reminder (if pending)
//! └── Diagnostics (real-time)
//! ```

use std::collections::HashMap;

/// SessionStart hook content builder
pub struct SessionStartHook {
    /// Skills with mandatory invocation
    mandatory_skills: Vec<String>,
    /// Whether to include red flags
    include_red_flags: bool,
    /// Whether to include skill priority rules
    include_skill_priority: bool,
    /// Custom hook content
    custom_content: Option<String>,
}

impl Default for SessionStartHook {
    fn default() -> Self {
        Self {
            mandatory_skills: Vec::new(),
            include_red_flags: true,
            include_skill_priority: true,
            custom_content: None,
        }
    }
}

impl SessionStartHook {
    /// Create a new SessionStart hook
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a mandatory skill
    pub fn add_mandatory_skill(mut self, skill_name: impl Into<String>) -> Self {
        self.mandatory_skills.push(skill_name.into());
        self
    }

    /// Set whether to include red flags
    pub fn with_red_flags(mut self, include: bool) -> Self {
        self.include_red_flags = include;
        self
    }

    /// Set custom hook content
    pub fn with_custom_content(mut self, content: impl Into<String>) -> Self {
        self.custom_content = Some(content.into());
        self
    }

    /// Build the hook content
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // 1. Mandatory skills warning
        if !self.mandatory_skills.is_empty() {
            parts.push(self.build_mandatory_skills_warning());
        }

        // 2. Red flags table
        if self.include_red_flags {
            parts.push(RED_FLAGS_SECTION.to_string());
        }

        // 3. Skill priority rules
        if self.include_skill_priority {
            parts.push(SKILL_PRIORITY_SECTION.to_string());
        }

        // 4. Custom content
        if let Some(ref content) = self.custom_content {
            parts.push(content.clone());
        }

        parts.join("\n\n")
    }

    fn build_mandatory_skills_warning(&self) -> String {
        let skills_list = self.mandatory_skills.join(", ");
        format!(
            "<EXTREMELY-IMPORTANT>
The following skills are **MANDATORY** and must be invoked before any response:
{}

If you think there's even a 1% chance these skills apply, you **MUST** invoke them first.
This is not negotiable. This is not optional.
</EXTREMELY-IMPORTANT>",
            skills_list
        )
    }
}

/// Red flags section content
const RED_FLAGS_SECTION: &str = "
## Red Flags - STOP and reconsider

These thoughts mean STOP — you're rationalizing:

| Thought | Reality |
|---------|---------|
| \"This is just a simple question\" | Questions are tasks. Check for skills. |
| \"I need more context first\" | Skill check comes BEFORE clarifying questions. |
| \"Let me explore the codebase first\" | Skills tell you HOW to explore. Check first. |
| \"I can check git/files quickly\" | Files lack conversation context. Check for skills. |
| \"Let me gather information first\" | Skills tell you HOW to gather information. |
| \"This doesn't need a formal skill\" | If a skill exists, use it. |
| \"I remember this skill\" | Skills evolve. Read current version. |
| \"This doesn't count as a task\" | Action = task. Check for skills. |
| \"The skill is overkill\" | Simple things become complex. Use it. |
| \"I'll just do this one thing first\" | Check BEFORE doing anything. |
| \"This feels productive\" | Undisciplined action wastes time. Skills prevent this. |
| \"I know what that means\" | Knowing the concept ≠ using the skill. Invoke it. |

**When you catch yourself thinking these, STOP and invoke the relevant skill.**
";

/// Skill priority section content
const SKILL_PRIORITY_SECTION: &str = "
## Skill Priority

When multiple skills could apply, use this order:

1. **Process skills first** (brainstorming, debugging, planning)
   - These determine HOW to approach the task
   - Must be invoked first

2. **Implementation skills second** (code-review, frontend-design)
   - These guide execution
   - After process skills

Examples:
- \"Let's build X\" → brainstorming → frontend-design
- \"Fix this bug\" → debugging → domain-specific skills
";

// ============================================================================
// Todo Reminder System
// ============================================================================

/// Todo reminder content builder
pub struct TodoReminder {
    /// Pending tasks
    pending_tasks: Vec<String>,
    /// In-progress task (at most one)
    in_progress: Option<String>,
    /// Reminder count (to track repeated reminders)
    reminder_count: HashMap<String, usize>,
    /// Max reminders per task
    max_reminders: usize,
}

impl Default for TodoReminder {
    fn default() -> Self {
        Self {
            pending_tasks: Vec::new(),
            in_progress: None,
            reminder_count: HashMap::new(),
            max_reminders: 2,
        }
    }
}

impl TodoReminder {
    /// Create a new todo reminder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set pending tasks
    pub fn set_pending_tasks(mut self, tasks: Vec<String>) -> Self {
        self.pending_tasks = tasks;
        self
    }

    /// Set in-progress task
    pub fn set_in_progress(mut self, task: impl Into<String>) -> Self {
        self.in_progress = Some(task.into());
        self
    }

    /// Set max reminders per task
    pub fn with_max_reminders(mut self, max: usize) -> Self {
        self.max_reminders = max;
        self
    }

    /// Check if should remind (not exceeded max)
    pub fn should_remind(&self, task: &str) -> bool {
        let count = self.reminder_count.get(task).copied().unwrap_or(0);
        count < self.max_reminders
    }

    /// Increment reminder count
    pub fn increment_reminder(&mut self, task: &str) {
        *self.reminder_count.entry(task.to_string()).or_insert(0) += 1;
    }

    /// Build reminder content
    pub fn build(&self) -> Option<String> {
        // Only remind if there's pending work and not exceeded max
        if self.pending_tasks.is_empty() && self.in_progress.is_none() {
            return None;
        }

        let mut lines = Vec::new();

        // In-progress task
        if let Some(ref task) = self.in_progress {
            lines.push(format!("⏳ **In Progress**: {}", task));
        }

        // Pending tasks (filter by reminder count)
        let remindable_pending: Vec<_> = self.pending_tasks
            .iter()
            .filter(|t| self.should_remind(t))
            .collect();

        if !remindable_pending.is_empty() {
            lines.push("\n📋 **Pending Tasks**:".to_string());
            for task in remindable_pending {
                lines.push(format!("  - {}", task));
            }
        }

        if lines.is_empty() {
            return None;
        }

        Some(format!(
            "<todo-reminder>\n{}\n</todo-reminder>",
            lines.join("\n")
        ))
    }
}

// ============================================================================
// Diagnostics Injection
// ============================================================================

/// Diagnostic entry for injection
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    /// File path
    pub file: String,
    /// Line number
    pub line: usize,
    /// Severity (error, warning, info)
    pub severity: String,
    /// Message
    pub message: String,
    /// Source (rustc, rust-analyzer, etc.)
    pub source: String,
}

/// Diagnostics injection builder
pub struct DiagnosticsInjection {
    /// Diagnostic entries
    diagnostics: Vec<DiagnosticEntry>,
    /// Max entries to show
    max_entries: usize,
}

impl Default for DiagnosticsInjection {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
            max_entries: 20,
        }
    }
}

impl DiagnosticsInjection {
    /// Create a new diagnostics injection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a diagnostic
    pub fn add_diagnostic(mut self, entry: DiagnosticEntry) -> Self {
        self.diagnostics.push(entry);
        self
    }

    /// Set diagnostics
    pub fn set_diagnostics(mut self, entries: Vec<DiagnosticEntry>) -> Self {
        self.diagnostics = entries;
        self
    }

    /// Set max entries
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Build diagnostics content
    pub fn build(&self) -> Option<String> {
        if self.diagnostics.is_empty() {
            return None;
        }

        // Limit entries
        let entries: Vec<_> = self.diagnostics.iter()
            .take(self.max_entries)
            .collect();

        let mut lines = Vec::new();
        lines.push("<new-diagnostics>".to_string());
        lines.push("The following new diagnostic issues were detected:".to_string());
        lines.push("\n".to_string());

        for diag in entries {
            let severity_marker = match diag.severity.as_str() {
                "error" => "✘",
                "warning" => "⚠",
                "info" => "ℹ",
                _ => "•",
            };

            lines.push(format!(
                "{} {}:{} {} [{}]",
                severity_marker,
                diag.file,
                diag.line,
                diag.message,
                diag.source
            ));
        }

        lines.push("\n</new-diagnostics>".to_string());

        Some(lines.join("\n"))
    }

    /// Check if has errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == "error")
    }

    /// Check if has warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == "warning")
    }
}

// ============================================================================
// Combined Session Context
// ============================================================================

/// Combined session start context
pub struct SessionStartContext {
    /// Session start hook
    pub hook: SessionStartHook,
    /// Todo reminder
    pub todo: TodoReminder,
    /// Diagnostics
    pub diagnostics: DiagnosticsInjection,
}

impl Default for SessionStartContext {
    fn default() -> Self {
        Self {
            hook: SessionStartHook::new(),
            todo: TodoReminder::new(),
            diagnostics: DiagnosticsInjection::new(),
        }
    }
}

impl SessionStartContext {
    /// Create a new session start context
    pub fn new() -> Self {
        Self::default()
    }

    /// Build all session start content
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // 1. Session start hook content
        let hook_content = self.hook.build();
        if !hook_content.is_empty() {
            parts.push(format!(
                "SessionStart hook additional context:\n{}",
                hook_content
            ));
        }

        // 2. Todo reminder
        if let Some(todo_content) = self.todo.build() {
            parts.push(todo_content);
        }

        // 3. Diagnostics
        if let Some(diag_content) = self.diagnostics.build() {
            parts.push(diag_content);
        }

        parts.join("\n\n")
    }

    /// Check if has any content to inject
    pub fn has_content(&self) -> bool {
        !self.hook.build().is_empty()
            || self.todo.build().is_some()
            || self.diagnostics.build().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_start_hook_builds_content() {
        let hook = SessionStartHook::new()
            .add_mandatory_skill("code-review")
            .with_red_flags(true);

        let content = hook.build();
        assert!(content.contains("EXTREMELY-IMPORTANT"));
        assert!(content.contains("code-review"));
        assert!(content.contains("Red Flags"));
    }

    #[test]
    fn test_session_start_hook_without_red_flags() {
        let hook = SessionStartHook::new()
            .with_red_flags(false);

        let content = hook.build();
        assert!(!content.contains("Red Flags"));
    }

    #[test]
    fn test_todo_reminder_with_pending_tasks() {
        let reminder = TodoReminder::new()
            .set_pending_tasks(vec!["Task A".to_string(), "Task B".to_string()]);

        let content = reminder.build();
        assert!(content.is_some());
        let content = content.unwrap();
        assert!(content.contains("Pending Tasks"));
        assert!(content.contains("Task A"));
    }

    #[test]
    fn test_todo_reminder_empty() {
        let reminder = TodoReminder::new();

        let content = reminder.build();
        assert!(content.is_none());
    }

    #[test]
    fn test_todo_reminder_max_limit() {
        let mut reminder = TodoReminder::new()
            .set_pending_tasks(vec!["Task A".to_string()])
            .with_max_reminders(2);

        // First two reminders should work
        assert!(reminder.should_remind("Task A"));
        reminder.increment_reminder("Task A");
        assert!(reminder.should_remind("Task A"));
        reminder.increment_reminder("Task A");

        // Third should not
        assert!(!reminder.should_remind("Task A"));
    }

    #[test]
    fn test_diagnostics_injection_with_errors() {
        let injection = DiagnosticsInjection::new()
            .add_diagnostic(DiagnosticEntry {
                file: "src/main.rs".to_string(),
                line: 42,
                severity: "error".to_string(),
                message: "missing semicolon".to_string(),
                source: "rustc".to_string(),
            });

        let content = injection.build();
        assert!(content.is_some());
        let content = content.unwrap();
        assert!(content.contains("new-diagnostics"));
        assert!(content.contains("✘"));
        assert!(content.contains("missing semicolon"));
    }

    #[test]
    fn test_diagnostics_has_errors() {
        let injection = DiagnosticsInjection::new()
            .add_diagnostic(DiagnosticEntry {
                file: "src/main.rs".to_string(),
                line: 42,
                severity: "error".to_string(),
                message: "error".to_string(),
                source: "rustc".to_string(),
            });

        assert!(injection.has_errors());
        assert!(!injection.has_warnings());
    }

    #[test]
    fn test_session_start_context_combined() {
        let hook = SessionStartHook::new().add_mandatory_skill("test");
        let todo = TodoReminder::new().set_pending_tasks(vec!["Task".to_string()]);
        let diagnostics = DiagnosticsInjection::new().add_diagnostic(DiagnosticEntry {
            file: "test.rs".to_string(),
            line: 1,
            severity: "warning".to_string(),
            message: "test".to_string(),
            source: "rustc".to_string(),
        });

        let context = SessionStartContext {
            hook,
            todo,
            diagnostics,
        };

        let content = context.build();
        assert!(content.contains("SessionStart hook"));
        assert!(content.contains("todo-reminder"));
        assert!(content.contains("new-diagnostics"));
    }
}