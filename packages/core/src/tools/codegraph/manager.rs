//! CodeGraph index manager.

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(not(target_os = "windows"))]
use tokio::process::Command;
use tokio::time::timeout;

use super::git::{get_git_status_changes, is_git_repository, update_version_after_sync};
use super::install::{ensure_codegraph, get_codegraph_path};
use super::project::find_project_root;
use super::types::{FileInfo, IndexStatus, Node, PendingChanges};
use crate::constants::CODEGRAPH_CLI_TIMEOUT_SECS;
use crate::memory::ProjectStructureAnalyzer;

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

    /// Create manager with automatic project root detection.
    pub fn with_auto_detect(start_path: &Path) -> Self {
        let project_path = find_project_root(start_path);
        Self::new(&project_path)
    }

    /// Check if CodeGraph is initialized for this project.
    pub fn is_initialized(&self) -> bool {
        self.db_path.exists()
    }

    /// Get SQLite connection (read-only for safety).
    pub fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA query_only = ON;")?;
        Ok(conn)
    }

    /// Initialize CodeGraph index via CLI.
    pub async fn init(&self) -> Result<()> {
        self.run_cli_command(&["init", "-i"]).await?;
        Ok(())
    }

    /// Reinitialize CodeGraph - delete old index and rebuild.
    pub async fn reinit(&self) -> Result<()> {
        if get_codegraph_path().is_none() {
            return Err(anyhow::anyhow!(
                "CodeGraph CLI not installed. Please install first."
            ));
        }

        let codegraph_dir = self.project_path.join(".codegraph");
        if codegraph_dir.exists() {
            log::info!("Deleting old index at {}", codegraph_dir.display());
            std::fs::remove_dir_all(&codegraph_dir)?;
        }

        log::info!("Rebuilding index for {}", self.project_path.display());
        self.init().await?;
        self.sync().await?;
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
        let codegraph_path =
            get_codegraph_path().ok_or_else(|| anyhow::anyhow!("CodeGraph CLI not installed"))?;

        timeout(Duration::from_secs(CODEGRAPH_CLI_TIMEOUT_SECS), async {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;

                let mut std_cmd = std::process::Command::new(&codegraph_path);
                std_cmd
                    .args(args)
                    .current_dir(&self.project_path)
                    .creation_flags(CREATE_NO_WINDOW);

                let result = std_cmd.output()?;
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    return Err(anyhow::anyhow!("CodeGraph command failed: {}", stderr));
                }
                Ok::<_, anyhow::Error>(())
            }

            #[cfg(not(target_os = "windows"))]
            {
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
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("CodeGraph CLI timeout"))?
    }

    /// Initialize CodeGraph for this project (check CLI and auto-install if needed).
    pub async fn ensure_initialized(&self) -> Result<()> {
        ensure_codegraph().await?;

        let analyzer = ProjectStructureAnalyzer::new(self.project_path.clone());
        if analyzer.detect_project_type().is_none() {
            return Err(anyhow::anyhow!(
                "Not a code project directory: {}",
                self.project_path.display()
            ));
        }

        if !self.is_initialized() {
            log::info!(
                "Initializing CodeGraph for: {}",
                self.project_path.display()
            );
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
             LIMIT ?",
        )?;

        let pattern = format!("%{}%", pattern);
        let nodes = stmt
            .query_map(params![&pattern, &pattern, limit], |row| {
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

    /// Find callers of a symbol.
    pub fn callers(&self, symbol_id: &str, limit: usize) -> Result<Vec<Node>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.signature, n.docstring, n.visibility, n.is_exported, n.is_async
             FROM nodes n
             INNER JOIN edges e ON n.id = e.source
             WHERE e.target = ? AND e.kind = 'calls'
             LIMIT ?",
        )?;

        let nodes = stmt
            .query_map(params![symbol_id, limit], |row| {
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

    /// Find callees of a symbol.
    pub fn callees(&self, symbol_id: &str, limit: usize) -> Result<Vec<Node>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.signature, n.docstring, n.visibility, n.is_exported, n.is_async
             FROM nodes n
             INNER JOIN edges e ON n.id = e.target
             WHERE e.source = ? AND e.kind = 'calls'
             LIMIT ?",
        )?;

        let nodes = stmt
            .query_map(params![symbol_id, limit], |row| {
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

        let mut stmt = conn.prepare("SELECT DISTINCT language FROM nodes")?;
        let languages: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IndexStatus {
            initialized: true,
            file_count,
            node_count,
            edge_count,
            languages,
            pending_changes: if is_git_repository(&self.project_path) {
                let changes = get_git_status_changes(&self.project_path);
                PendingChanges {
                    added: changes.added.len() as u32,
                    modified: changes.modified.len() as u32,
                    removed: changes.deleted.len() as u32,
                }
            } else {
                PendingChanges {
                    added: 0,
                    modified: 0,
                    removed: 0,
                }
            },
        })
    }

    /// Get files by language.
    pub fn files(&self, language: Option<&str>) -> Result<Vec<FileInfo>> {
        let conn = self.connect()?;

        // Query files with metadata from the files table
        let mut stmt = if let Some(_lang) = language {
            conn.prepare("SELECT path, language, 0 as size, 0 as modified, node_count FROM files WHERE language = ?")?
        } else {
            conn.prepare("SELECT path, language, 0 as size, 0 as modified, node_count FROM files")?
        };

        let files = if let Some(lang) = language {
            stmt.query_map(params![lang], |row| {
                Ok(FileInfo {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    size: row.get(2)?,
                    modified: row.get(3)?,
                    node_count: Some(row.get(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], |row| {
                Ok(FileInfo {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    size: row.get(2)?,
                    modified: row.get(3)?,
                    node_count: Some(row.get(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        Ok(files)
    }

    // ========================================================================
    // CLI-Based Query Methods (explore, node, impact, affected)
    // ========================================================================

    /// Explore an area: relevant symbols' source + call paths.
    /// Returns Markdown output from CodeGraph CLI.
    pub async fn explore(&self, query: &str, max_files: Option<usize>) -> Result<String> {
        if let Some(max) = max_files {
            let max_str = max.to_string();
            self.run_cli_command_output(&["explore", query, "--max-files", &max_str]).await
        } else {
            self.run_cli_command_output(&["explore", query]).await
        }
    }

    /// Get one symbol's source + caller/callee trail.
    /// Returns Markdown output from CodeGraph CLI.
    pub async fn node(&self, name: &str, file: Option<&str>) -> Result<String> {
        if let Some(f) = file {
            self.run_cli_command_output(&["node", name, "-f", f]).await
        } else {
            self.run_cli_command_output(&["node", name]).await
        }
    }

    /// Analyze what code is affected by changing a symbol.
    /// Returns ANSI-colored output from CodeGraph CLI.
    pub async fn impact(&self, symbol: &str, depth: Option<usize>) -> Result<String> {
        if let Some(d) = depth {
            let depth_str = d.to_string();
            self.run_cli_command_output(&["impact", symbol, "-d", &depth_str]).await
        } else {
            self.run_cli_command_output(&["impact", symbol]).await
        }
    }

    /// Find test files affected by changed source files.
    /// Returns plain text output from CodeGraph CLI.
    pub async fn affected(&self, files: &[&str], depth: Option<usize>) -> Result<String> {
        // Build args as String to avoid lifetime issues
        let mut args: Vec<String> = vec!["affected".to_string()];
        if let Some(d) = depth {
            args.push("-d".to_string());
            args.push(d.to_string());
        }
        for f in files {
            args.push(f.to_string());
        }

        // Convert to Vec<&str> for the CLI call
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_cli_command_output(&args_refs).await
    }

    /// Run CodeGraph CLI command and capture output.
    async fn run_cli_command_output(&self, args: &[&str]) -> Result<String> {
        let codegraph_path =
            get_codegraph_path().ok_or_else(|| anyhow::anyhow!("CodeGraph CLI not installed"))?;

        timeout(Duration::from_secs(CODEGRAPH_CLI_TIMEOUT_SECS), async {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;

                let mut std_cmd = std::process::Command::new(&codegraph_path);
                std_cmd
                    .args(args)
                    .current_dir(&self.project_path)
                    .creation_flags(CREATE_NO_WINDOW);

                let result = std_cmd.output()?;
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    return Err(anyhow::anyhow!("CodeGraph command failed: {}", stderr));
                }
                Ok::<_, anyhow::Error>(String::from_utf8_lossy(&result.stdout).to_string())
            }

            #[cfg(not(target_os = "windows"))]
            {
                let result = Command::new(&codegraph_path)
                    .args(args)
                    .current_dir(&self.project_path)
                    .output()
                    .await?;

                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    return Err(anyhow::anyhow!("CodeGraph command failed: {}", stderr));
                }
                Ok::<_, anyhow::Error>(String::from_utf8_lossy(&result.stdout).to_string())
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("CodeGraph CLI timeout"))?
    }
}
