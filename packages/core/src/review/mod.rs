//! Pre-write review system for code quality assurance.
//!
//! This module provides automatic code review before write/edit/multi_edit operations
//! to ensure code quality and prevent low-quality code from entering the project.

pub mod ai_review;
pub mod context;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Context information for enhanced review
#[derive(Debug, Clone, Default)]
pub struct ReviewContext {
    /// LSP diagnostics for the file (errors, warnings)
    pub lsp_diagnostics: Vec<LspDiagnostic>,
    /// CodeGraph symbols defined in the file
    pub symbols: Vec<SymbolInfo>,
    /// Callers of symbols being modified
    pub callers: Vec<String>,
    /// Callees (dependencies) of symbols being modified
    pub callees: Vec<String>,
    /// Project memory context (relevant patterns, decisions)
    pub memory_context: Option<String>,
    /// Related files that might be affected
    pub related_files: Vec<String>,
}

/// LSP diagnostic information
#[derive(Debug, Clone)]
pub struct LspDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<u32>,
    pub source: Option<String>,
}

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    pub fn icon(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "❌",
            DiagnosticSeverity::Warning => "⚠️",
            DiagnosticSeverity::Information => "ℹ️",
            DiagnosticSeverity::Hint => "💡",
        }
    }
}

/// Symbol information from CodeGraph
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: Option<String>,
    pub doc: Option<String>,
}

/// Symbol kind
#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Constant,
    Module,
    Interface,
    Type,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Method => "method",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Module => "module",
            SymbolKind::Interface => "interface",
            SymbolKind::Type => "type",
        }
    }
}

/// Input for pre-write review
#[derive(Debug, Clone)]
pub struct PreWriteReviewInput {
    pub tool_name: String,
    pub file_path: PathBuf,
    pub existing_content: Option<String>,
    pub new_content: String,
    pub edit_info: Option<EditInfo>,
    /// Enhanced context for better review
    pub context: ReviewContext,
}

/// Edit operation details
#[derive(Debug, Clone)]
pub struct EditInfo {
    pub old_string: String,
    pub new_string: String,
}

/// Review result from AI analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreWriteReviewResult {
    /// Overall score (0-100)
    pub overall_score: u8,
    /// Issues found during review
    pub issues: Vec<ReviewIssue>,
    /// Impact analysis
    pub impact_analysis: ImpactAnalysis,
    /// Improvement suggestions
    pub suggestions: Vec<String>,
}

/// Issue found during review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    /// Issue level (Critical, Warning, Suggestion)
    pub level: IssueLevel,
    /// Issue category
    pub category: IssueCategory,
    /// Issue description
    pub message: String,
    /// Code location (line number or function name)
    pub location: Option<String>,
    /// Fix example (for critical issues)
    pub fix_example: Option<String>,
}

/// Issue severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueLevel {
    Critical,
    Warning,
    Suggestion,
}

impl IssueLevel {
    pub fn icon(&self) -> &'static str {
        match self {
            IssueLevel::Critical => "🔴",
            IssueLevel::Warning => "🟡",
            IssueLevel::Suggestion => "🟢",
        }
    }
}

/// Issue category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueCategory {
    Quality,
    Security,
    Performance,
    Impact,
    Practice,
}

impl IssueCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueCategory::Quality => "Quality",
            IssueCategory::Security => "Security",
            IssueCategory::Performance => "Performance",
            IssueCategory::Impact => "Impact",
            IssueCategory::Practice => "Practice",
        }
    }
}

/// Impact analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub affected_modules: Vec<String>,
    pub dependencies: Vec<String>,
    pub breaking_changes: bool,
}

impl Default for ImpactAnalysis {
    fn default() -> Self {
        Self {
            affected_modules: vec!["unknown".to_string()],
            dependencies: vec![],
            breaking_changes: false,
        }
    }
}

impl PreWriteReviewInput {
    pub fn from_tool_input(tool_name: &str, input: &serde_json::Value) -> Result<Self> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' in tool input"))?;
        
        let file_path = PathBuf::from(path_str);
        let existing_content = if file_path.exists() {
            std::fs::read_to_string(&file_path).ok()
        } else {
            None
        };
        
        let (new_content, edit_info) = match tool_name {
            "write" => {
                let content = input["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?
                    .to_string();
                (content, None)
            }
            "edit" => {
                let old = input["old_string"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'old_string'"))?
                    .to_string();
                let new = input["new_string"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'new_string'"))?
                    .to_string();
                let new_content = existing_content.as_ref()
                    .map(|e| e.replace(&old, &new))
                    .unwrap_or_else(|| new.clone());
                (new_content, Some(EditInfo { old_string: old, new_string: new }))
            }
            "multi_edit" => {
                // multi_edit has array of edits: {"path": "...", "edits": [{old_string, new_string}, ...]}
                let edits = input["edits"].as_array()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edits' array in multi_edit"))?;
                
                let mut new_content = existing_content.clone().unwrap_or_default();
                for edit_item in edits {
                    let old = edit_item["old_string"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing 'old_string' in edit"))?;
                    let new = edit_item["new_string"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing 'new_string' in edit"))?;
                    new_content = new_content.replace(old, new);
                }
                (new_content, None) // Multiple edits, no single edit info
            }
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };
        
        Ok(PreWriteReviewInput {
            tool_name: tool_name.to_string(),
            file_path,
            existing_content,
            new_content,
            edit_info,
            context: ReviewContext::default(),
        })
    }
    
    /// Create input with enhanced context
    pub fn with_context(mut self, context: ReviewContext) -> Self {
        self.context = context;
        self
    }
}

impl PreWriteReviewResult {
    pub fn should_write(&self) -> bool {
        self.overall_score >= 60 && !self.issues.iter().any(|i| i.level == IssueLevel::Critical)
    }
}

pub fn format_review_summary(result: &PreWriteReviewResult) -> String {
    let critical = result.issues.iter().filter(|i| i.level == IssueLevel::Critical).count();
    let warning = result.issues.iter().filter(|i| i.level == IssueLevel::Warning).count();
    let suggestion = result.issues.iter().filter(|i| i.level == IssueLevel::Suggestion).count();
    
    format!(
        "审核结果: 评分 {} | 🔴 {} | 🟡 {} | 🟢 {}",
        result.overall_score, critical, warning, suggestion
    )
}

pub fn format_review_report(result: &PreWriteReviewResult) -> String {
    let mut output = format_review_summary(result);
    output.push_str("\n\n问题详情:\n");
    
    for issue in &result.issues {
        output.push_str(&format!(
            "{} {}: {}\n",
            issue.level.icon(),
            issue.category.as_str(),
            issue.message
        ));
    }
    
    output
}