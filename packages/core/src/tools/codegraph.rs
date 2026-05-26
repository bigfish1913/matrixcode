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
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, timeout};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::cancel::CancellationToken;
use crate::constants::{CODEGRAPH_CLI_TIMEOUT_SECS, CODEGRAPH_SYNC_INTERVAL_SECS};

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

/// Ensure CodeGraph is available (check and auto-install if needed).
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
            let mut cmd = Command::new(&codegraph_path);
            cmd.args(args)
                .current_dir(&self.project_path)
                .output()
                .await?;

            if !cmd.status().await?.success() {
                return Err(anyhow::anyhow!("CodeGraph command failed"));
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

    /// Run the watcher loop.
    async fn run_watcher_loop(
        project_path: PathBuf,
        sync_interval: Duration,
        cancel_token: CancellationToken,
    ) {
        // Ensure CodeGraph CLI is available
        if get_codegraph_path().is_none() {
            log::warn!("CodeGraph CLI not found, watcher will not auto-sync");
            // Try to install
            if let Err(e) = install_codegraph().await {
                log::warn!("CodeGraph auto-install failed: {}", e);
                return;
            }
        }

        // Initialize CodeGraph if not already
        let manager = CodeGraphManager::new(&project_path);
        if !manager.is_initialized() {
            log::info!("Initializing CodeGraph for: {}", project_path.display());
            if let Err(e) = manager.init().await {
                log::warn!("CodeGraph init failed: {}", e);
                return;
            }
        }

        // Channel for file change events
        let (change_tx, mut change_rx) = mpsc::channel::<PathBuf>(100);

        // Create file watcher
        let watcher_result = Self::create_file_watcher(&project_path, change_tx);

        if watcher_result.is_err() {
            log::warn!("CodeGraph watcher failed to start: {}", watcher_result.err().unwrap());
            return;
        }

        let _watcher = watcher_result.unwrap();

        // Load ignore matcher with .gitignore support
        let ignore_matcher = IgnoreMatcher::load(&project_path);

        // Track last sync time for debouncing
        let mut last_sync = Instant::now();
        let mut pending_changes = false;

        log::info!("CodeGraph watcher started for: {}", project_path.display());

        loop {
            // Check cancellation first
            if cancel_token.is_cancelled() {
                log::info!("CodeGraph watcher stopped (cancelled)");
                break;
            }

            tokio::select! {
                // Check for file changes
                Some(path) = change_rx.recv() => {
                    // Check if it's a source file and not ignored
                    if is_source_file(&path)
                        && !ignore_matcher.should_ignore(&path, &project_path) {
                        pending_changes = true;
                        log::debug!("CodeGraph: source file changed: {}", path.display());
                    }
                }

                // Periodic sync check (debounced)
                _ = sleep(sync_interval) => {
                    if pending_changes && last_sync.elapsed() >= sync_interval {
                        // Run sync
                        let manager = CodeGraphManager::new(&project_path);
                        if manager.is_initialized() {
                            log::info!("CodeGraph: auto-sync triggered");
                            if let Err(e) = manager.sync().await {
                                log::warn!("CodeGraph sync failed: {}", e);
                            }
                            last_sync = Instant::now();
                            pending_changes = false;
                        }
                    }
                }
            }
        }
    }

    /// Create the underlying file watcher.
    fn create_file_watcher(
        project_path: &Path,
        change_tx: mpsc::Sender<PathBuf>,
    ) -> Result<RecommendedWatcher> {
        let tx = change_tx.clone();

        let handler = move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                for path in event.paths {
                    // Send change event
                    let _ = tx.blocking_send(path);
                }
            }
        };

        let mut watcher = RecommendedWatcher::new(handler, Config::default())?;
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
            name: "codegraph_search".to_string(),
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
            name: "codegraph_callers".to_string(),
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
            name: "codegraph_callees".to_string(),
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
            name: "codegraph_status".to_string(),
            description: "检查 CodeGraph 索引状态。返回文件数、节点数、边数、支持的语言等信息。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        let status = self.manager.status()?;

        if !status.initialized {
            return Ok("CodeGraph 未初始化。\n\n运行 'codegraph init -i' 来构建代码索引。".to_string());
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

// ============================================================================
// Helper Functions
// ============================================================================

/// Create all CodeGraph tools for a project.
pub fn codegraph_tools(project_path: &Path) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CodeGraphSearchTool::new(project_path)),
        Box::new(CodeGraphCallersTool::new(project_path)),
        Box::new(CodeGraphCalleesTool::new(project_path)),
        Box::new(CodeGraphStatusTool::new(project_path)),
    ]
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
        assert!(names.contains(&"codegraph_search".to_string()));
        assert!(names.contains(&"codegraph_callers".to_string()));
        assert!(names.contains(&"codegraph_callees".to_string()));
        assert!(names.contains(&"codegraph_status".to_string()));
    }

    #[test]
    fn test_search_tool_priority() {
        let path = PathBuf::from(".");
        let tools = codegraph_tools(&path);

        for tool in tools {
            let def = tool.definition();
            if def.name == "codegraph_search" {
                assert!(def.is_priority);
            }
        }
    }
}