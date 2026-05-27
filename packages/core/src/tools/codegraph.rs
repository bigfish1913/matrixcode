//! CodeGraph tool for semantic code analysis.
//!
//! Integrates CodeGraph (https://github.com/colbymchenry/codegraph) for:
//! - Symbol search across the codebase
//! - Call graph analysis (callers/callees)
//! - Impact analysis for code changes
//! - Task context building for AI agents
//!
//! Uses SQLite direct access for fast queries, and CLI for index building.
//!
//! # Auto-sync
//!
//! CodeGraphWatcher provides automatic file watching and index synchronization.
//! When source files change, it automatically runs `codegraph sync` to keep
//! the index up-to-date.

use anyhow::Result;
use async_trait::async_trait;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{sleep, timeout};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::cancel::CancellationToken;
use crate::constants::{CODEGRAPH_CLI_TIMEOUT_SECS, CODEGRAPH_SYNC_INTERVAL_SECS};
use crate::memory::ProjectStructureAnalyzer;

// ============================================================================
// Data Structures
// ============================================================================

/// Code symbol node from CodeGraph index.
#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub language: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub visibility: Option<String>,
    pub is_exported: bool,
    pub is_async: bool,
}

/// Edge representing relationship between nodes.
#[derive(Debug, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub line: Option<u32>,
}

/// Index status information.
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStatus {
    pub initialized: bool,
    pub file_count: u32,
    pub node_count: u32,
    pub edge_count: u32,
    pub languages: Vec<String>,
    pub pending_changes: PendingChanges,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingChanges {
    pub added: u32,
    pub modified: u32,
    pub removed: u32,
}

// ============================================================================
// CodeGraph CLI Detection and Installation
// ============================================================================

/// Get CodeGraph installation directory (platform-specific).
fn get_codegraph_install_dir() -> Option<PathBuf> {
    // Use dirs crate to get platform-appropriate local data directory
    dirs::data_local_dir()
        .map(|p| p.join("codegraph").join("current").join("bin"))
}

/// Get CodeGraph CLI executable name (platform-specific).
fn get_codegraph_exe_name() -> String {
    if cfg!(windows) {
        "codegraph.cmd".to_string()
    } else {
        "codegraph".to_string()
    }
}

/// Check if CodeGraph CLI is installed.
pub fn is_codegraph_installed() -> bool {
    // Try direct command (in PATH)
    if std::process::Command::new("codegraph")
        .arg("--version")
        .output()
        .is_ok() {
        return true;
    }

    // Try platform-specific installation path
    if let Some(install_dir) = get_codegraph_install_dir() {
        let exe_name = get_codegraph_exe_name();
        let exe_path = install_dir.join(&exe_name);
        if exe_path.exists()
            && std::process::Command::new(&exe_path)
                .arg("--version")
                .output()
                .is_ok() {
                return true;
            }
    }

    false
}

/// Get CodeGraph CLI path (returns the executable path or command name).
pub fn get_codegraph_path() -> Option<String> {
    // Try direct command first (in PATH)
    if std::process::Command::new("codegraph")
        .arg("--version")
        .output()
        .is_ok() {
        return Some("codegraph".to_string());
    }

    // Try platform-specific installation path
    if let Some(install_dir) = get_codegraph_install_dir() {
        let exe_name = get_codegraph_exe_name();
        let exe_path = install_dir.join(&exe_name);
        if exe_path.exists() {
            return Some(exe_path.to_string_lossy().to_string());
        }
    }

    None
}

/// Auto-install CodeGraph CLI (Windows).
pub async fn install_codegraph() -> Result<()> {
    log::info!("Installing CodeGraph CLI...");

    // Windows PowerShell installer
    let result = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "irm https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.ps1 | iex"
        ])
        .output()
        .await?;

    if result.status.success() {
        log::info!("CodeGraph CLI installed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(anyhow::anyhow!("CodeGraph installation failed: {}", stderr))
    }
}

/// CodeGraph installation status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeGraphInstallStatus {
    /// Already installed and available.
    Installed(String),
    /// Not installed, needs user approval to install.
    NotInstalled,
}

/// Check CodeGraph installation status (no auto-install).
pub fn check_codegraph_status() -> CodeGraphInstallStatus {
    match get_codegraph_path() {
        Some(path) => CodeGraphInstallStatus::Installed(path),
        None => CodeGraphInstallStatus::NotInstalled,
    }
}

/// Ensure CodeGraph is available with optional auto-install.
pub async fn ensure_codegraph() -> Result<String> {
    if let Some(path) = get_codegraph_path() {
        return Ok(path);
    }

    // Auto-install
    install_codegraph().await?;

    // Check again after installation
    get_codegraph_path()
        .ok_or_else(|| anyhow::anyhow!("CodeGraph installation failed - please install manually"))
}

/// Ensure CodeGraph with user prompt support.
/// Returns None if not installed and user declined installation.
pub async fn ensure_codegraph_with_prompt(prompt_fn: impl FnOnce() -> bool) -> Result<Option<String>> {
    match get_codegraph_path() {
        Some(path) => Ok(Some(path)),
        None => {
            // Prompt user for installation
            if prompt_fn() {
                install_codegraph().await?;
                get_codegraph_path()
                    .ok_or_else(|| anyhow::anyhow!("CodeGraph installation failed - please install manually"))
                    .map(Some)
            } else {
                Ok(None)
            }
        }
    }
}

// ============================================================================
// CodeGraph Manager
// ============================================================================

/// Manages CodeGraph index for a project.
pub struct CodeGraphManager {
    project_path: PathBuf,
    db_path: PathBuf,
}

impl CodeGraphManager {
    /// Create manager for a project path.
    pub fn new(project_path: &Path) -> Self {
        let db_path = project_path.join(".codegraph").join("codegraph.db");
        Self {
            project_path: project_path.to_path_buf(),
            db_path,
        }
    }

    /// Check if CodeGraph is initialized for this project.
    pub fn is_initialized(&self) -> bool {
        self.db_path.exists()
    }

    /// Get SQLite connection (read-only for safety).
    pub fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        // Enable read-only mode
        conn.execute_batch("PRAGMA query_only = ON;")?;
        Ok(conn)
    }

    /// Initialize CodeGraph index via CLI.
    pub async fn init(&self) -> Result<()> {
        self.run_cli_command(&["init", "-i"]).await?;
        Ok(())
    }

    /// Reinitialize CodeGraph - delete old index, matrix.md and rebuild.
    /// Returns error if CodeGraph CLI is not installed.
    pub async fn reinit(&self) -> Result<()> {
        // Check if CodeGraph CLI is available
        if get_codegraph_path().is_none() {
            return Err(anyhow::anyhow!(
                "CodeGraph CLI not installed. Please install first or use init with --install flag."
            ));
        }

        // Delete old .codegraph directory
        let codegraph_dir = self.project_path.join(".codegraph");
        if codegraph_dir.exists() {
            log::info!("CodeGraph: deleting old index at {}", codegraph_dir.display());
            std::fs::remove_dir_all(&codegraph_dir)?;
        }

        // Delete matrix.md overview file (if exists)
        let matrix_md_path = self.project_path.join(".matrix").join("matrix.md");
        if matrix_md_path.exists() {
            log::info!("CodeGraph: deleting old matrix.md at {}", matrix_md_path.display());
            std::fs::remove_file(&matrix_md_path)?;
        }

        // Rebuild fresh index
        log::info!("CodeGraph: rebuilding index for {}", self.project_path.display());
        self.init().await?;

        // Sync to ensure everything is up to date
        self.sync().await?;

        // Save version after reinit
        update_version_after_sync(&self.project_path);

        Ok(())
    }

    /// Sync index with latest file changes.
    pub async fn sync(&self) -> Result<()> {
        self.run_cli_command(&["sync"]).await?;
        Ok(())
    }

    /// Run codegraph CLI command.
    async fn run_cli_command(&self, args: &[&str]) -> Result<()> {
        let codegraph_path = get_codegraph_path()
            .ok_or_else(|| anyhow::anyhow!("CodeGraph CLI not installed. Run 'codegraph install' or use matrixcode to auto-install."))?;

        timeout(Duration::from_secs(CODEGRAPH_CLI_TIMEOUT_SECS), async {
            let result = Command::new(&codegraph_path)
                .args(args)
                .current_dir(&self.project_path)
                .output()
                .await?;

            if !result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                return Err(anyhow::anyhow!("CodeGraph command failed: {}", stderr));
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!(format!("CodeGraph CLI timeout ({})s", CODEGRAPH_CLI_TIMEOUT_SECS)))?
    }

    /// Initialize CodeGraph for this project (check CLI and auto-install if needed).
    pub async fn ensure_initialized(&self) -> Result<()> {
        // Ensure CLI is installed
        ensure_codegraph().await?;

        // Check if this is a code project
        let analyzer = ProjectStructureAnalyzer::new(self.project_path.clone());
        if analyzer.detect_project_type().is_none() {
            return Err(anyhow::anyhow!(
                "Not a code project directory: {}. CodeGraph requires a project with Cargo.toml, package.json, go.mod, etc.",
                self.project_path.display()
            ));
        }

        // Initialize if not already
        if !self.is_initialized() {
            log::info!("Initializing CodeGraph for: {}", self.project_path.display());
            self.init().await?;
        }

        Ok(())
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Search symbols by name pattern.
    pub fn search(&self, pattern: &str, limit: usize) -> Result<Vec<Node>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, language,
                    start_line, end_line, start_column, end_column,
                    signature, docstring, visibility, is_exported, is_async
             FROM nodes
             WHERE name LIKE ? OR qualified_name LIKE ?
             ORDER BY name
             LIMIT ?"
        )?;

        let pattern = format!("%{}%", pattern);
        let nodes = stmt.query_map(params![&pattern, &pattern, limit], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: row.get(5)?,
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                signature: row.get(10)?,
                docstring: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? != 0,
                is_async: row.get::<_, i32>(14)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    /// Find callers of a symbol (what calls this function).
    pub fn callers(&self, symbol_id: &str, limit: usize) -> Result<Vec<Node>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.signature, n.docstring, n.visibility, n.is_exported, n.is_async
             FROM nodes n
             INNER JOIN edges e ON n.id = e.source
             WHERE e.target = ? AND e.kind = 'calls'
             LIMIT ?"
        )?;

        let nodes = stmt.query_map(params![symbol_id, limit], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: row.get(5)?,
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                signature: row.get(10)?,
                docstring: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? != 0,
                is_async: row.get::<_, i32>(14)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    /// Find callees of a symbol (what this function calls).
    pub fn callees(&self, symbol_id: &str, limit: usize) -> Result<Vec<Node>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.signature, n.docstring, n.visibility, n.is_exported, n.is_async
             FROM nodes n
             INNER JOIN edges e ON n.id = e.target
             WHERE e.source = ? AND e.kind = 'calls'
             LIMIT ?"
        )?;

        let nodes = stmt.query_map(params![symbol_id, limit], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: row.get(5)?,
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                signature: row.get(10)?,
                docstring: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? != 0,
                is_async: row.get::<_, i32>(14)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    /// Get index status.
    pub fn status(&self) -> Result<IndexStatus> {
        if !self.is_initialized() {
            return Ok(IndexStatus {
                initialized: false,
                file_count: 0,
                node_count: 0,
                edge_count: 0,
                languages: vec![],
                pending_changes: PendingChanges {
                    added: 0,
                    modified: 0,
                    removed: 0,
                },
            });
        }

        let conn = self.connect()?;

        let file_count: u32 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        let node_count: u32 = conn.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))?;
        let edge_count: u32 = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))?;

        // Get unique languages
        let mut stmt = conn.prepare("SELECT DISTINCT language FROM nodes")?;
        let languages: Vec<String> = stmt.query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IndexStatus {
            initialized: true,
            file_count,
            node_count,
            edge_count,
            languages,
            pending_changes: PendingChanges {
                added: 0,
                modified: 0,
                removed: 0,
            },
        })
    }

    /// Get files by language.
    pub fn files(&self, language: Option<&str>) -> Result<Vec<FileInfo>> {
        let conn = self.connect()?;
        let mut stmt = if let Some(_lang) = language {
            conn.prepare(
                "SELECT path, language, node_count FROM files WHERE language = ?"
            )?
        } else {
            conn.prepare("SELECT path, language, node_count FROM files")?
        };

        let files = if let Some(lang) = language {
            stmt.query_map(params![lang], |row| {
                Ok(FileInfo {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    node_count: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], |row| {
                Ok(FileInfo {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    node_count: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        Ok(files)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub language: String,
    pub node_count: u32,
}

// ============================================================================
// File Watcher with Smart Filtering
// ============================================================================

/// Patterns to ignore for file watching (similar to CodeGraph defaults).
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    // Build outputs
    "target", "dist", "build", "out", "bin", "obj", ".output",
    // Dependencies
    "node_modules", "vendor", "Pods", ".venv", "venv", "__pycache__",
    // Cache and temp
    ".cache", ".tmp", ".temp", "tmp", "temp",
    // IDE and tools
    ".idea", ".vscode", ".eclipse", ".project", ".classpath",
    // Generated files
    ".generated", "generated", ".codegraph",
    // Lock files
    "package-lock.json", "yarn.lock", "Cargo.lock", "pnpm-lock.yaml",
    // Test outputs
    "coverage", ".nyc_output", "test-results",
    // Logs
    "logs",
];

/// Extensions to watch (source files only).
const WATCH_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go",
    "java", "kt", "kts", "c", "cpp", "cc", "h", "hpp",
    "rb", "php", "swift", "cs", "scala", "lua", "sh",
];

/// Git status polling interval (for non-fsmonitor fallback)
const GIT_STATUS_POLL_INTERVAL_SECS: u64 = 2;

// ============================================================================
// Git Environment Detection & Helpers
// ============================================================================

/// Check if directory is inside a Git work tree.
fn is_git_repository(project_path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current Git HEAD commit SHA.
fn get_git_head_sha(project_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Get all Git tracked files (for efficient init).
#[allow(dead_code)]
fn get_git_tracked_files(project_path: &Path) -> Vec<PathBuf> {
    std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(project_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .filter_map(|line| {
                            let path = project_path.join(line);
                            if is_source_file(&path) {
                                Some(path)
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Get changed files via git status --porcelain.
/// Returns (modified, added, deleted) file lists.
fn get_git_status_changes(project_path: &Path) -> GitStatusChanges {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_path)
        .output();

    let mut changes = GitStatusChanges::default();

    if let Ok(o) = output
        && o.status.success() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if line.len() < 2 {
                continue;
            }
            let status = &line[..2];
            let path = line[3..].trim();

            // Handle rename format: "R100 old -> new"
            let file_path = if path.contains(" -> ") {
                path.split(" -> ").nth(1).unwrap_or(path)
            } else {
                path
            };

            let full_path = project_path.join(file_path);

            // Check if it's a source file
            if !is_source_file(&full_path) {
                continue;
            }

            // Categorize by status code
            match status.trim() {
                "M" | "MM" | "AM" => changes.modified.push(full_path),
                "A" | "??" => changes.added.push(full_path),
                "D" | "AD" | "MD" => changes.deleted.push(full_path),
                "R" => {
                    // Rename: treat as delete old + add new
                    if let Some(old_path) = path.split(" -> ").next() {
                        changes.deleted.push(project_path.join(old_path));
                    }
                    changes.added.push(full_path);
                }
                _ => {}
            }
        }
    }

    changes
}

/// Start Git fsmonitor daemon (if available).
fn start_git_fsmonitor(project_path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["fsmonitor--daemon", "start"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Git fsmonitor daemon is running.
fn is_git_fsmonitor_running(project_path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["fsmonitor--daemon", "status"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Git status changes result.
#[derive(Debug, Default)]
struct GitStatusChanges {
    modified: Vec<PathBuf>,
    added: Vec<PathBuf>,
    deleted: Vec<PathBuf>,
}

impl GitStatusChanges {
    fn has_changes(&self) -> bool {
        !self.modified.is_empty() || !self.added.is_empty() || !self.deleted.is_empty()
    }

    #[allow(dead_code)]
    fn total_count(&self) -> usize {
        self.modified.len() + self.added.len() + self.deleted.len()
    }
}

// ============================================================================
// Version Tracking (Git SHA Persistence)
// ============================================================================

/// Version file name for storing Git HEAD SHA.
const VERSION_FILE: &str = "version.txt";

/// Save current Git HEAD SHA to version file.
fn save_version_sha(project_path: &Path, sha: &str) {
    let version_path = project_path.join(".codegraph").join(VERSION_FILE);
    if let Some(parent) = version_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&version_path, sha);
}

/// Load stored Git HEAD SHA from version file.
fn load_version_sha(project_path: &Path) -> Option<String> {
    let version_path = project_path.join(".codegraph").join(VERSION_FILE);
    std::fs::read_to_string(&version_path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Check if version has changed (current SHA != stored SHA).
fn has_version_changed(project_path: &Path) -> bool {
    let current_sha = get_git_head_sha(project_path);
    let stored_sha = load_version_sha(project_path);
    current_sha != stored_sha
}

/// Update version after successful sync.
fn update_version_after_sync(project_path: &Path) {
    if let Some(sha) = get_git_head_sha(project_path) {
        save_version_sha(project_path, &sha);
        log::debug!("CodeGraph: version updated to SHA {}", sha);
    }
}

/// Environment type for CodeGraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeGraphEnv {
    Git,
    NonGit,
}

/// Detect environment type at startup.
fn detect_env_type(project_path: &Path) -> CodeGraphEnv {
    if is_git_repository(project_path) {
        CodeGraphEnv::Git
    } else {
        CodeGraphEnv::NonGit
    }
}

/// Gitignore patterns loaded from file.
pub struct IgnoreMatcher {
    patterns: Vec<String>,
    negation_patterns: Vec<String>,
}

impl IgnoreMatcher {
    /// Load ignore patterns from .gitignore and defaults.
    pub fn load(project_path: &Path) -> Self {
        let mut patterns = Vec::new();
        let mut negation_patterns = Vec::new();

        // Add default patterns
        for p in DEFAULT_IGNORE_PATTERNS {
            patterns.push(p.to_string());
        }

        // Load .gitignore
        let gitignore_path = project_path.join(".gitignore");
        if gitignore_path.exists()
            && let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(stripped) = line.strip_prefix('!') {
                        // Negation pattern (include this)
                        negation_patterns.push(stripped.to_string());
                    } else {
                        patterns.push(line.to_string());
                    }
                }
            }

        Self { patterns, negation_patterns }
    }

    /// Check if a path should be ignored.
    pub fn should_ignore(&self, path: &Path, project_path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let relative_path = path.strip_prefix(project_path)
            .unwrap_or(path)
            .to_string_lossy();

        // Check negation patterns first (explicit inclusion)
        for pattern in &self.negation_patterns {
            if Self::matches_pattern(&relative_path, pattern) {
                return false; // Explicitly included
            }
        }

        // Check ignore patterns
        for pattern in &self.patterns {
            if Self::matches_pattern(&relative_path, pattern)
                || path_str.contains(pattern) {
                return true;
            }
        }

        // Check hidden files (but allow .codegraph)
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.')
                    && name_str != ".codegraph"
                    && !WATCH_EXTENSIONS.contains(&name_str.split('.').next_back().unwrap_or("")) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if path matches a gitignore pattern.
    fn matches_pattern(path: &str, pattern: &str) -> bool {
        // Simple pattern matching (handles common gitignore patterns)
        let pattern = pattern.trim_start_matches('/');

        // Directory match (pattern ends with /)
        if let Some(dir_pattern) = pattern.strip_suffix('/') {
            return path.contains(dir_pattern) || path.starts_with(dir_pattern);
        }

        // Wildcard match
        if pattern.contains('*') {
            let parts = pattern.split('*').collect::<Vec<_>>();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return (prefix.is_empty() || path.starts_with(prefix))
                    && (suffix.is_empty() || path.ends_with(suffix));
            }
        }

        // Exact match or contains
        path == pattern || path.contains(pattern) || path.starts_with(&format!("{}/", pattern))
    }
}

/// Check if a path is a source file worth watching.
fn is_source_file(path: &Path) -> bool {
    // Check extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        return WATCH_EXTENSIONS.contains(&ext_str.as_str());
    }
    false
}

/// CodeGraph file watcher for auto-sync.
pub struct CodeGraphWatcher {
    project_path: PathBuf,
    stop_tx: broadcast::Sender<()>,
    sync_interval: Duration,
}

impl CodeGraphWatcher {
    /// Create a new watcher for the project.
    pub fn new(project_path: &Path) -> Self {
        let (stop_tx, _) = broadcast::channel(1);
        Self {
            project_path: project_path.to_path_buf(),
            stop_tx,
            sync_interval: Duration::from_secs(CODEGRAPH_SYNC_INTERVAL_SECS), // Debounce interval
        }
    }

    /// Start watching for file changes.
    /// Returns a stop handle that can be used to stop the watcher.
    pub fn start(&self, cancel_token: CancellationToken) -> Result<broadcast::Receiver<()>> {
        let stop_rx = self.stop_tx.subscribe();
        let project_path = self.project_path.clone();
        let sync_interval = self.sync_interval;

        // Spawn watcher task
        tokio::spawn(async move {
            Self::run_watcher_loop(project_path, sync_interval, cancel_token).await;
        });

        Ok(stop_rx)
    }

    /// Stop the watcher.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    /// Run the watcher loop with dual-path monitoring.
    async fn run_watcher_loop(
        project_path: PathBuf,
        _sync_interval: Duration,
        cancel_token: CancellationToken,
    ) {
        // Check if CodeGraph CLI is available (no auto-install)
        if get_codegraph_path().is_none() {
            log::warn!("CodeGraph CLI not found, watcher disabled. Please install CodeGraph manually.");
            return;
        }

        // Check if this is a code project
        let analyzer = ProjectStructureAnalyzer::new(project_path.clone());
        if analyzer.detect_project_type().is_none() {
            log::info!(
                "CodeGraph: skipping non-code directory: {}",
                project_path.display()
            );
            return;
        }

        // Detect environment type
        let env_type = detect_env_type(&project_path);
        log::info!(
            "CodeGraph: environment detected as {} for: {}",
            match env_type {
                CodeGraphEnv::Git => "Git repository",
                CodeGraphEnv::NonGit => "non-Git directory",
            },
            project_path.display()
        );

        // Initialize CodeGraph if not already
        let manager = CodeGraphManager::new(&project_path);
        if !manager.is_initialized() {
            log::info!("Initializing CodeGraph for: {}", project_path.display());
            if let Err(e) = manager.init().await {
                log::warn!("CodeGraph init failed: {}", e);
                return;
            }
            // Initial sync after init
            if let Err(e) = manager.sync().await {
                log::warn!("CodeGraph post-init sync failed: {}", e);
            }
            // Save version after init
            update_version_after_sync(&project_path);
        }

        // Check version consistency before starting
        if env_type == CodeGraphEnv::Git && has_version_changed(&project_path) {
            log::info!("CodeGraph: version changed, performing sync before starting watcher");
            if let Err(e) = manager.sync().await {
                log::warn!("CodeGraph version sync failed: {}", e);
            }
            update_version_after_sync(&project_path);
        }

        // Initial sync on startup
        log::info!("CodeGraph: performing initial sync on startup");
        if let Err(e) = manager.sync().await {
            log::warn!("CodeGraph initial sync failed: {}", e);
        }
        update_version_after_sync(&project_path);

        // Channel for file change events (from notify)
        let (change_tx, mut change_rx) = mpsc::channel::<PathBuf>(100);

        // Create notify file watcher (always running as fallback)
        let watcher_result = Self::create_file_watcher(&project_path, change_tx.clone());
        if watcher_result.is_err() {
            log::warn!("CodeGraph notify watcher failed to start: {}", watcher_result.err().unwrap());
            return;
        }
        let _watcher = watcher_result.unwrap();

        // Load ignore matcher
        let ignore_matcher = IgnoreMatcher::load(&project_path);

        // Track sync state with deduplication (async-safe)
        let syncing = Arc::new(AtomicBool::new(false));
        let syncing_clone = syncing.clone();
        let changed_files = Arc::new(RwLock::new(std::collections::HashSet::<PathBuf>::new()));
        let last_change = Arc::new(std::sync::Mutex::new(Instant::now()));

        // Debounce settings
        let debounce_delay = Duration::from_secs(CODEGRAPH_SYNC_INTERVAL_SECS);
        let git_poll_interval = Duration::from_secs(GIT_STATUS_POLL_INTERVAL_SECS);

        // Start Git monitoring if in Git environment
        let git_monitoring = if env_type == CodeGraphEnv::Git {
            // Try to start Git fsmonitor daemon
            if start_git_fsmonitor(&project_path) {
                log::info!("CodeGraph: Git fsmonitor daemon started");
                true
            } else if is_git_fsmonitor_running(&project_path) {
                log::info!("CodeGraph: Git fsmonitor daemon already running");
                true
            } else {
                log::info!("CodeGraph: Git fsmonitor not available, using git status polling");
                false
            }
        } else {
            false
        };

        log::info!(
            "CodeGraph watcher started (Git monitoring: {}, notify fallback: always)",
            git_monitoring
        );

        // Check interval
        let check_interval = Duration::from_secs(1);

        loop {
            if cancel_token.is_cancelled() {
                // Final sync before exit
                let pending_count = changed_files.read().await.len();
                if pending_count > 0 {
                    log::info!(
                        "CodeGraph: final sync before exit ({} unique files)",
                        pending_count
                    );
                    let manager = CodeGraphManager::new(&project_path);
                    if manager.is_initialized() {
                        let _ = manager.sync().await;
                        update_version_after_sync(&project_path);
                    }
                }
                log::info!("CodeGraph watcher stopped");
                break;
            }

            tokio::select! {
                // Notify file changes (fallback path - always running)
                Some(path) = change_rx.recv() => {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    if is_source_file(&path)
                        && !ignore_matcher.should_ignore(&path, &project_path) {
                        // Add to set (deduplicated, async-safe)
                        {
                            let mut files = changed_files.write().await;
                            if files.insert(path.clone()) {
                                *last_change.lock().unwrap() = Instant::now();
                                log::debug!(
                                    "CodeGraph [notify]: new file {} (total unique: {})",
                                    path.display(),
                                    files.len()
                                );
                            }
                        }
                    }
                }

                // Git status polling (Git environment only)
                _ = sleep(git_poll_interval), if git_monitoring => {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    // Check Git status for changes
                    let changes = get_git_status_changes(&project_path);
                    if changes.has_changes() {
                        let mut new_count = 0;
                        {
                            let mut files = changed_files.write().await;
                            // Add all changed files to set (deduplicated)
                            for path in changes.modified.iter().chain(&changes.added).chain(&changes.deleted) {
                                if files.insert(path.clone()) {
                                    new_count += 1;
                                }
                            }
                            if new_count > 0 {
                                log::debug!(
                                    "CodeGraph [git]: {} new changes (modified: {}, added: {}, deleted: {}, total unique: {})",
                                    new_count,
                                    changes.modified.len(),
                                    changes.added.len(),
                                    changes.deleted.len(),
                                    files.len()
                                );
                            }
                        }
                        if new_count > 0 {
                            *last_change.lock().unwrap() = Instant::now();
                        }
                    }
                }

                // Periodic sync check (debounced)
                _ = sleep(check_interval) => {
                    if cancel_token.is_cancelled() {
                        break;
                    }

                    let files_count = changed_files.read().await.len();
                    let elapsed = last_change.lock().unwrap().elapsed();

                    // Sync when: not syncing + have pending + debounce elapsed
                    if !syncing_clone.load(Ordering::SeqCst)
                        && files_count > 0
                        && elapsed >= debounce_delay {
                        syncing_clone.store(true, Ordering::SeqCst);
                        log::info!("CodeGraph: auto-sync triggered ({} unique files changed)", files_count);

                        let manager = CodeGraphManager::new(&project_path);
                        if manager.is_initialized() {
                            if let Err(e) = manager.sync().await {
                                log::warn!("CodeGraph sync failed: {}", e);
                            } else {
                                update_version_after_sync(&project_path);
                            }
                            // Clear the set after sync (async-safe)
                            changed_files.write().await.clear();
                        }
                        syncing_clone.store(false, Ordering::SeqCst);
                    }
                }
            }
        }
    }

    /// Create the underlying file watcher with optimized config.
    fn create_file_watcher(
        project_path: &Path,
        change_tx: mpsc::Sender<PathBuf>,
    ) -> Result<RecommendedWatcher> {
        let tx = change_tx.clone();

        let handler = move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                // Only process create/modify/remove events, ignore access/other
                if !event.kind.is_access() && !event.kind.is_other() {
                    for path in event.paths {
                        // Send change event (non-blocking to avoid stalls)
                        let _ = tx.try_send(path);
                    }
                }
            }
        };

        // Use optimized config to reduce event noise
        let config = Config::default()
            .with_poll_interval(Duration::from_secs(2)) // Reduce poll frequency
            .with_compare_contents(false); // Don't compare file contents

        let mut watcher = RecommendedWatcher::new(handler, config)?;
        watcher.watch(project_path, RecursiveMode::Recursive)?;

        Ok(watcher)
    }
}

// ============================================================================
// Tool Definitions
// ============================================================================

/// Tool for searching symbols in CodeGraph index.
pub struct CodeGraphSearchTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphSearchTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_search".to_string(),
            description: "搜索代码符号（函数、类、方法等）。比 grep 更快，返回符号定义位置和签名信息。需要项目已初始化 CodeGraph (.codegraph/ 目录存在)。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "符号名称搜索模式（支持模糊匹配）"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 20）",
                        "default": 20
                    }
                },
                "required": ["pattern"]
            }),
            is_priority: true,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        if !self.manager.is_initialized() {
            return Ok("CodeGraph 未初始化。请先运行 codegraph init -i 来构建索引。".to_string());
        }

        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?;
        let limit = params["limit"].as_u64().unwrap_or(20) as usize;

        let nodes = self.manager.search(pattern, limit)?;

        if nodes.is_empty() {
            return Ok(format!("未找到匹配 '{}' 的符号。", pattern));
        }

        let mut results = Vec::new();
        for node in nodes {
            let sig = node.signature.as_deref().unwrap_or("");
            results.push(format!(
                "{} {} ({})\n  {}:{}\n  {}",
                node.kind, node.name, node.language,
                node.file_path, node.start_line,
                sig
            ));
        }

        Ok(results.join("\n\n"))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for finding callers of a symbol.
pub struct CodeGraphCallersTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphCallersTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphCallersTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_callers".to_string(),
            description: "查找调用指定符号的所有函数/方法。用于理解代码依赖关系，分析修改影响范围。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "符号 ID 或名称"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 10）",
                        "default": 10
                    }
                },
                "required": ["symbol"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        if !self.manager.is_initialized() {
            return Ok("CodeGraph 未初始化。请先运行 codegraph init -i 来构建索引。".to_string());
        }

        let symbol = params["symbol"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'symbol'"))?;
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        // First try to find the symbol ID
        let symbol_id = if symbol.contains(":") {
            symbol.to_string()
        } else {
            let nodes = self.manager.search(symbol, 1)?;
            if nodes.is_empty() {
                return Ok(format!("未找到符号 '{}'。", symbol));
            }
            nodes[0].id.clone()
        };

        let callers = self.manager.callers(&symbol_id, limit)?;

        if callers.is_empty() {
            return Ok(format!("符号 '{}' 没有调用者。", symbol));
        }

        let mut results = Vec::new();
        for node in callers {
            results.push(format!(
                "{} {} ({})\n  {}:{}",
                node.kind, node.name, node.language,
                node.file_path, node.start_line
            ));
        }

        Ok(format!("调用 '{}' 的符号：\n\n{}", symbol, results.join("\n")))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for finding callees of a symbol.
pub struct CodeGraphCalleesTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphCalleesTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphCalleesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_callees".to_string(),
            description: "查找指定符号调用的所有函数/方法。用于理解函数执行流程和依赖链。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "符号 ID 或名称"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 10）",
                        "default": 10
                    }
                },
                "required": ["symbol"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        if !self.manager.is_initialized() {
            return Ok("CodeGraph 未初始化。请先运行 codegraph init -i 来构建索引。".to_string());
        }

        let symbol = params["symbol"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'symbol'"))?;
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        let symbol_id = if symbol.contains(":") {
            symbol.to_string()
        } else {
            let nodes = self.manager.search(symbol, 1)?;
            if nodes.is_empty() {
                return Ok(format!("未找到符号 '{}'。", symbol));
            }
            nodes[0].id.clone()
        };

        let callees = self.manager.callees(&symbol_id, limit)?;

        if callees.is_empty() {
            return Ok(format!("符号 '{}' 不调用其他符号。", symbol));
        }

        let mut results = Vec::new();
        for node in callees {
            results.push(format!(
                "{} {} ({})\n  {}:{}",
                node.kind, node.name, node.language,
                node.file_path, node.start_line
            ));
        }

        Ok(format!("'{}' 调用的符号：\n\n{}", symbol, results.join("\n")))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for checking CodeGraph index status.
pub struct CodeGraphStatusTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphStatusTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_status".to_string(),
            description: "检查 CodeGraph 索引状态。返回文件数、节点数、边数、支持的语言等信息。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        // Check if CodeGraph CLI is installed first
        if get_codegraph_path().is_none() {
            return Ok("CodeGraph CLI 未安装。\n\n请先安装 CodeGraph CLI:\n- Windows: 运行 PowerShell 安装脚本\n- Linux/Mac: 运行安装脚本\n\n安装后运行 'codegraph init -i' 来构建代码索引。".to_string());
        }

        let status = self.manager.status()?;

        if !status.initialized {
            return Ok("CodeGraph 未初始化。\n\n运行 'codegraph init -i' 来构建代码索引，或在 matrixcode 中使用 /init 命令。".to_string());
        }

        Ok(format!(
            "CodeGraph 状态：\n\n文件数: {}\n节点数: {}\n边数: {}\n语言: {}",
            status.file_count,
            status.node_count,
            status.edge_count,
            status.languages.join(", ")
        ))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for manually syncing CodeGraph index.
pub struct CodeGraphSyncTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphSyncTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphSyncTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_sync".to_string(),
            description: "手动同步 CodeGraph 索引。当代码库有变化但自动同步未触发时使用，确保搜索结果是最新的。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        if !self.manager.is_initialized() {
            return Ok("CodeGraph 未初始化。请先运行 codegraph init -i 来构建索引。".to_string());
        }

        log::info!("CodeGraph: manual sync triggered by AI");
        self.manager.sync().await?;

        let status = self.manager.status()?;
        Ok(format!(
            "CodeGraph 索引已同步。\n\n文件数: {}\n节点数: {}\n边数: {}\n语言: {}",
            status.file_count,
            status.node_count,
            status.edge_count,
            status.languages.join(", ")
        ))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create all CodeGraph tools for a project.
/// Create CodeGraph tools for a project path.
/// Always returns tools - they will show appropriate error messages
/// if CodeGraph is not installed or initialized.
pub fn codegraph_tools(project_path: &Path) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CodeGraphSearchTool::new(project_path)),
        Box::new(CodeGraphCallersTool::new(project_path)),
        Box::new(CodeGraphCalleesTool::new(project_path)),
        Box::new(CodeGraphStatusTool::new(project_path)),
        Box::new(CodeGraphSyncTool::new(project_path)),
    ]
}

/// Check if CodeGraph tools should be injected.
/// Returns true if CodeGraph CLI is installed (even if not initialized).
pub fn should_inject_codegraph_tools() -> bool {
    get_codegraph_path().is_some()
}

/// Create CodeGraph tools only if installed.
/// Returns empty vec if CodeGraph CLI is not available.
pub fn codegraph_tools_if_installed(project_path: &Path) -> Vec<Box<dyn Tool>> {
    if should_inject_codegraph_tools() {
        codegraph_tools(project_path)
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_codegraph_manager_creation() {
        let path = PathBuf::from(".");
        let manager = CodeGraphManager::new(&path);
        assert!(manager.db_path.to_str().unwrap().contains(".codegraph"));
    }

    #[test]
    fn test_tool_definitions() {
        let path = PathBuf::from(".");
        let tools = codegraph_tools(&path);

        let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();
        assert!(names.contains(&"code_search".to_string()));
        assert!(names.contains(&"code_callers".to_string()));
        assert!(names.contains(&"code_callees".to_string()));
        assert!(names.contains(&"code_status".to_string()));
        assert!(names.contains(&"code_sync".to_string()));
    }

    #[test]
    fn test_search_tool_priority() {
        let path = PathBuf::from(".");
        let tools = codegraph_tools(&path);

        for tool in tools {
            let def = tool.definition();
            if def.name == "code_search" {
                assert!(def.is_priority);
            }
        }
    }
}