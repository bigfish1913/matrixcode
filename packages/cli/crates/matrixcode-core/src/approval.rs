//! Approval gate: interactive confirmation before executing mutating or dangerous tools.
//!
//! Three modes:
//! - `Auto`: execute everything without asking (trust the AI).
//! - `Ask` (default): pause before mutating/dangerous operations.
//! - `Strict`: pause before every tool call.

use std::fmt;
use std::io::{self, BufRead, Write as _};

use serde_json::Value;

// ============================================================================
// Risk Level
// ============================================================================

/// Risk level assigned to each tool operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// Read-only, no side effects (e.g., read, search, glob, ls).
    Safe,
    /// Modifies files but in a controlled way (e.g., write, edit, multi_edit, todo_write).
    Mutating,
    /// Potentially dangerous or irreversible (e.g., bash commands).
    Dangerous,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "safe"),
            RiskLevel::Mutating => write!(f, "mutating"),
            RiskLevel::Dangerous => write!(f, "dangerous"),
        }
    }
}

impl RiskLevel {
    /// Get the icon symbol for this risk level.
    pub fn icon(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "ℹ️ ",
            RiskLevel::Mutating => "📝",
            RiskLevel::Dangerous => "⚠️ ",
        }
    }
}

// ============================================================================
// Approve Mode
// ============================================================================

/// Approval mode controlling when the user is prompted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ApproveMode {
    /// Never ask, execute everything automatically.
    Auto,
    /// Ask before mutating and dangerous operations (default).
    #[default]
    Ask,
    /// Ask before every tool call, including safe ones.
    Strict,
}

impl ApproveMode {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "auto" => ApproveMode::Auto,
            "strict" => ApproveMode::Strict,
            _ => ApproveMode::Ask,
        }
    }

    /// Cycle to the next mode: Ask -> Auto -> Strict -> Ask
    pub fn next(&self) -> Self {
        match self {
            ApproveMode::Ask => ApproveMode::Auto,
            ApproveMode::Auto => ApproveMode::Strict,
            ApproveMode::Strict => ApproveMode::Ask,
        }
    }
}

impl fmt::Display for ApproveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApproveMode::Auto => write!(f, "auto"),
            ApproveMode::Ask => write!(f, "ask"),
            ApproveMode::Strict => write!(f, "strict"),
        }
    }
}

// ============================================================================
// Approval Answer
// ============================================================================

/// User's response to an approval prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAnswer {
    /// Proceed with execution.
    Yes,
    /// Skip this tool call (return a "rejected" message to the AI).
    No,
    /// Abort the entire turn.
    Abort,
}

// ============================================================================
// Approval Request
// ============================================================================

/// A human-readable summary of what is about to happen.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub risk_level: RiskLevel,
    pub summary: String,
}

impl ApprovalRequest {
    /// Build an approval request from tool name, risk level, and parameters.
    pub fn new(tool_name: &str, risk: RiskLevel, params: &Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            risk_level: risk,
            summary: build_summary(tool_name, params),
        }
    }
}

/// Build a human-readable summary for the tool operation.
fn build_summary(tool_name: &str, params: &Value) -> String {
    match tool_name {
        "write" => summary_write(params),
        "edit" => summary_edit(params),
        "multi_edit" => summary_multi_edit(params),
        "bash" => summary_bash(params),
        "todo_write" => "更新任务清单".to_string(),
        _ => format!("执行工具: {}", tool_name),
    }
}

fn summary_write(params: &Value) -> String {
    let path = params["path"].as_str().unwrap_or("<unknown>");
    format!("写入文件: {}", path)
}

fn summary_edit(params: &Value) -> String {
    let path = params["path"].as_str().unwrap_or("<unknown>");
    format!("编辑文件: {}", path)
}

fn summary_multi_edit(params: &Value) -> String {
    let path = params["path"].as_str().unwrap_or("<unknown>");
    let count = params["edits"].as_array().map(|a| a.len()).unwrap_or(0);
    format!("批量编辑文件: {} ({} 处修改)", path, count)
}

fn summary_bash(params: &Value) -> String {
    let cmd = params["command"].as_str().unwrap_or("<unknown>");
    let display_cmd = if cmd.len() > 120 {
        let mut end = 120;
        while end > 0 && !cmd.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &cmd[..end])
    } else {
        cmd.to_string()
    };
    format!("执行命令: {}", display_cmd)
}

// ============================================================================
// Core Functions
// ============================================================================

/// Determine whether approval is needed given the mode and risk level.
pub fn needs_approval(mode: ApproveMode, risk: RiskLevel) -> bool {
    match mode {
        ApproveMode::Auto => false,
        ApproveMode::Ask => risk >= RiskLevel::Mutating,
        ApproveMode::Strict => true,
    }
}

/// Convenience function: build request and prompt user.
pub fn build_approval_request(tool_name: &str, risk: RiskLevel, params: &Value) -> ApprovalRequest {
    ApprovalRequest::new(tool_name, risk, params)
}

/// Display the approval prompt and wait for user input.
/// Returns the user's answer.
pub fn prompt_approval(request: &ApprovalRequest) -> ApprovalAnswer {
    println!();
    println!("┌─ 确认请求 ─────────────────────────────────────────");
    println!("│ {} {}", request.risk_level.icon(), request.summary);
    println!("│ 风险等级: {}", request.risk_level);
    println!("│");
    println!("│ [y] 执行  [n] 跳过  [a] 中止本轮");
    println!("└───���────────────────────────────────────────────────");
    print!("> ");
    let _ = io::stdout().flush();

    let answer = read_approval_answer();
    println!();
    answer
}

/// Read a single answer from stdin.
fn read_approval_answer() -> ApprovalAnswer {
    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return ApprovalAnswer::No;
    }
    match line.trim().to_lowercase().as_str() {
        "y" | "yes" | "" => ApprovalAnswer::Yes,
        "n" | "no" => ApprovalAnswer::No,
        "a" | "abort" | "q" | "quit" => ApprovalAnswer::Abort,
        _ => ApprovalAnswer::Yes, // default to yes for unrecognized input
    }
}