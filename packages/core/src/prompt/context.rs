//! Runtime Context Injection
//!
//! Provides dynamic context injection for:
//! - User context (CLAUDE.md files, preferences)
//! - System context (git status, date, workspace info)
//! - Environment context (platform, tools available)

use std::path::{Path, PathBuf};
use chrono::Local;
use serde::{Deserialize, Serialize};

/// User context from CLAUDE.md and preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    /// Content from CLAUDE.md file
    pub claude_md_content: Option<String>,
    /// User preferences (from accumulated memory)
    pub preferences: Vec<String>,
    /// Current date/time
    pub current_date: String,
    /// User's preferred language
    pub language: Option<String>,
}

impl Default for UserContext {
    fn default() -> Self {
        Self {
            claude_md_content: None,
            preferences: Vec::new(),
            current_date: Local::now().format("%Y-%m-%d").to_string(),
            language: None,
        }
    }
}

/// System context (git, workspace, tools)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemContext {
    /// Git branch name
    pub git_branch: Option<String>,
    /// Git status (clean/dirty)
    pub git_status: Option<String>,
    /// Current working directory
    pub working_directory: Option<String>,
    /// Project root directory
    pub project_root: Option<String>,
    /// Detected project type
    pub project_type: Option<ProjectType>,
    /// Available tools
    pub available_tools: Vec<String>,
    /// Platform info
    pub platform: String,
}

impl Default for SystemContext {
    fn default() -> Self {
        Self {
            git_branch: None,
            git_status: None,
            working_directory: None,
            project_root: None,
            project_type: None,
            available_tools: Vec::new(),
            platform: std::env::consts::OS.to_string(),
        }
    }
}

/// Detected project type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectType {
    Rust,
    NodeJs,
    Python,
    Go,
    Java,
    Cpp,
    Mixed,
    Unknown,
}

impl ProjectType {
    /// Detect project type from directory contents
    pub fn detect<P: AsRef<Path>>(dir: P) -> Self {
        let dir = dir.as_ref();
        
        let has_cargo = dir.join("Cargo.toml").exists();
        let has_package_json = dir.join("package.json").exists();
        let has_pyproject = dir.join("pyproject.toml").exists() || dir.join("setup.py").exists();
        let has_go_mod = dir.join("go.mod").exists();
        let has_pom = dir.join("pom.xml").exists() || dir.join("build.gradle").exists();
        let has_cmake = dir.join("CMakeLists.txt").exists() || dir.join("Makefile").exists();
        
        let types = [has_cargo, has_package_json, has_pyproject, has_go_mod, has_pom, has_cmake];
        let count = types.iter().filter(|&&x| x).count();
        
        if count > 1 {
            return ProjectType::Mixed;
        }
        
        if has_cargo {
            ProjectType::Rust
        } else if has_package_json {
            ProjectType::NodeJs
        } else if has_pyproject {
            ProjectType::Python
        } else if has_go_mod {
            ProjectType::Go
        } else if has_pom {
            ProjectType::Java
        } else if has_cmake {
            ProjectType::Cpp
        } else {
            ProjectType::Unknown
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectType::Rust => "rust",
            ProjectType::NodeJs => "nodejs",
            ProjectType::Python => "python",
            ProjectType::Go => "go",
            ProjectType::Java => "java",
            ProjectType::Cpp => "cpp",
            ProjectType::Mixed => "mixed",
            ProjectType::Unknown => "unknown",
        }
    }
}

/// Context injector for runtime context
pub struct ContextInjector {
    /// Working directory
    working_dir: PathBuf,
    /// Cache of user context
    user_context_cache: Option<UserContext>,
    /// Cache of system context
    system_context_cache: Option<SystemContext>,
    /// Whether to refresh cache
    dirty: bool,
}

impl ContextInjector {
    /// Create a new context injector
    pub fn new<P: Into<PathBuf>>(working_dir: P) -> Self {
        Self {
            working_dir: working_dir.into(),
            user_context_cache: None,
            system_context_cache: None,
            dirty: true,
        }
    }

    /// Mark cache as dirty (need refresh)
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Get user context
    pub fn get_user_context(&mut self) -> &UserContext {
        if self.dirty || self.user_context_cache.is_none() {
            self.user_context_cache = Some(self.collect_user_context());
            self.dirty = false;
        }
        self.user_context_cache.as_ref().unwrap()
    }

    /// Get system context
    pub fn get_system_context(&mut self) -> &SystemContext {
        if self.dirty || self.system_context_cache.is_none() {
            self.system_context_cache = Some(self.collect_system_context());
        }
        self.system_context_cache.as_ref().unwrap()
    }

    /// Collect user context
    fn collect_user_context(&self) -> UserContext {
        let mut ctx = UserContext::default();
        
        // Read CLAUDE.md
        let claude_md_path = self.working_dir.join("CLAUDE.md");
        if claude_md_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&claude_md_path) {
                ctx.claude_md_content = Some(content);
            }
        }
        
        // Also check parent directories
        if ctx.claude_md_content.is_none() {
            if let Some(parent) = self.working_dir.parent() {
                let parent_claude_md = parent.join("CLAUDE.md");
                if parent_claude_md.exists() {
                    if let Ok(content) = std::fs::read_to_string(&parent_claude_md) {
                        ctx.claude_md_content = Some(content);
                    }
                }
            }
        }
        
        ctx
    }

    /// Collect system context
    fn collect_system_context(&self) -> SystemContext {
        let mut ctx = SystemContext::default();
        
        // Git info
        if let Ok(output) = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.working_dir)
            .output()
        {
            if output.status.success() {
                ctx.git_branch = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
        
        if let Ok(output) = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.working_dir)
            .output()
        {
            if output.status.success() {
                let status = String::from_utf8_lossy(&output.stdout);
                ctx.git_status = if status.trim().is_empty() {
                    Some("clean".to_string())
                } else {
                    Some(format!("dirty ({} changes)", status.lines().count()))
                };
            }
        }
        
        // Working directory
        ctx.working_directory = self.working_dir.to_str().map(|s| s.to_string());
        ctx.project_root = ctx.working_directory.clone();
        
        // Project type
        ctx.project_type = Some(ProjectType::detect(&self.working_dir));
        
        // Available tools (check common tools)
        let tools = ["git", "cargo", "npm", "python", "go", "docker"];
        for tool in tools {
            if Self::tool_available(tool) {
                ctx.available_tools.push(tool.to_string());
            }
        }
        
        ctx
    }

    /// Check if a tool is available
    fn tool_available(tool: &str) -> bool {
        #[cfg(unix)]
        {
            std::process::Command::new("which")
                .arg(tool)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        
        #[cfg(windows)]
        {
            std::process::Command::new("where")
                .arg(tool)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }

    /// Render user context as prompt section
    pub fn render_user_context(&mut self) -> String {
        let ctx = self.get_user_context();
        let mut parts = Vec::new();
        
        // Date
        parts.push(format!("<currentDate>\n{}\n</currentDate>", ctx.current_date));
        
        // CLAUDE.md content
        if let Some(ref claude_md) = ctx.claude_md_content {
            parts.push(format!("<userPreferences>\n{}\n</userPreferences>", claude_md));
        }
        
        parts.join("\n\n")
    }

    /// Render system context as prompt section
    pub fn render_system_context(&mut self) -> String {
        let ctx = self.get_system_context();
        let mut parts = Vec::new();
        
        // Working directory
        if let Some(ref dir) = ctx.working_directory {
            parts.push(format!("<workingDirectory>\n{}\n</workingDirectory>", dir));
        }
        
        // Git info
        if let Some(ref branch) = ctx.git_branch {
            let git_info = if let Some(ref status) = ctx.git_status {
                format!("Branch: {}\nStatus: {}", branch, status)
            } else {
                format!("Branch: {}", branch)
            };
            parts.push(format!("<gitContext>\n{}\n</gitContext>", git_info));
        }
        
        // Project type
        if let Some(ref pt) = ctx.project_type {
            parts.push(format!("<projectType>\n{}\n</projectType>", pt.as_str()));
        }
        
        // Available tools
        if !ctx.available_tools.is_empty() {
            parts.push(format!("<availableTools>\n{}\n</availableTools>", ctx.available_tools.join(", ")));
        }
        
        parts.join("\n\n")
    }

    /// Render full context (for injection into prompt)
    pub fn render_full_context(&mut self) -> String {
        let user = self.render_user_context();
        let system = self.render_system_context();
        
        format!(
            "<context>\n{}\n\n{}\n</context>",
            user, system
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_user_context_default() {
        let ctx = UserContext::default();
        assert!(ctx.claude_md_content.is_none());
        assert!(!ctx.current_date.is_empty());
    }

    #[test]
    fn test_system_context_default() {
        let ctx = SystemContext::default();
        assert!(ctx.git_branch.is_none());
        assert!(!ctx.platform.is_empty());
    }

    #[test]
    fn test_project_type_detect_rust() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("Cargo.toml"), "").unwrap();
        
        assert_eq!(ProjectType::detect(temp_dir.path()), ProjectType::Rust);
    }

    #[test]
    fn test_project_type_detect_mixed() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("Cargo.toml"), "").unwrap();
        std::fs::write(temp_dir.path().join("package.json"), "").unwrap();
        
        assert_eq!(ProjectType::detect(temp_dir.path()), ProjectType::Mixed);
    }

    #[test]
    fn test_context_invalidator() {
        let mut injector = ContextInjector::new(std::env::current_dir().unwrap());
        
        // Get once
        let _ = injector.get_user_context();
        
        // Invalidate
        injector.invalidate();
        
        // Should refresh
        let _ = injector.get_user_context();
    }

    #[test]
    fn test_render_user_context() {
        let mut injector = ContextInjector::new(std::env::current_dir().unwrap());
        let rendered = injector.render_user_context();
        
        assert!(rendered.contains("<currentDate>"));
    }

    #[test]
    fn test_render_system_context() {
        let mut injector = ContextInjector::new(std::env::current_dir().unwrap());
        let rendered = injector.render_system_context();
        
        assert!(rendered.contains("<workingDirectory>") || rendered.contains("<projectType>"));
    }
}