//! File watcher for automatic index synchronization.

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::sleep;

use super::git::{
    check_mcp_daemon_active, check_sync_lock_owner, get_git_status_changes, has_version_changed,
    is_git_fsmonitor_running, is_git_repository, is_source_file, release_sync_lock,
    release_watcher_lock, start_git_fsmonitor, try_acquire_sync_lock, try_acquire_watcher_lock,
    update_version_after_sync, update_watcher_heartbeat,
};
use super::ignore::IgnoreMatcher;
use super::install::get_codegraph_path;
use super::manager::CodeGraphManager;
use super::project::find_project_root;
use super::types::CodeGraphEnv;
use crate::cancel::CancellationToken;
use crate::constants::CODEGRAPH_SYNC_INTERVAL_SECS;
use crate::memory::ProjectStructureAnalyzer;

/// Git status polling interval (for non-fsmonitor fallback).
const GIT_STATUS_POLL_INTERVAL_SECS: u64 = 2;

/// Handle to manage a running CodeGraph watcher.
/// Provides lifecycle management: start, stop, status check.
/// Internally uses Arc, so it can be cloned and shared across threads.
#[derive(Clone)]
pub struct WatcherHandle {
    handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    project_path: PathBuf,
}

impl WatcherHandle {
    /// Create a new handle (no watcher running yet).
    pub fn new(project_path: &Path) -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
            project_path: project_path.to_path_buf(),
        }
    }

    /// Create handle with automatic project root detection.
    pub fn with_auto_detect(start_path: &Path) -> Self {
        let project_path = find_project_root(start_path);
        log::info!(
            "CodeGraph: detected project root at {}",
            project_path.display()
        );
        Self::new(&project_path)
    }

    /// Check if watcher is currently running.
    pub fn is_running(&self) -> bool {
        let guard = self.handle.lock().unwrap();
        guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false)
    }

    /// Start watcher if not running and no daemon conflict.
    /// Returns true if watcher was started.
    pub fn start_if_needed(&self, cancel_token: CancellationToken) -> bool {
        if self.is_running() {
            log::info!("CodeGraph watcher already running");
            return false;
        }

        if CodeGraphWatcher::is_daemon_running(&self.project_path) {
            log::info!("CodeGraph MCP daemon detected, skipping watcher to avoid conflict");
            return false;
        }

        let watcher = CodeGraphWatcher::new(&self.project_path);
        let handle = watcher.start(cancel_token);
        log::info!("CodeGraph watcher started (no MCP daemon detected)");

        *self.handle.lock().unwrap() = Some(handle);
        true
    }

    /// Stop the watcher if running.
    pub fn stop(&self) {
        let guard = self.handle.lock().unwrap();
        if let Some(ref h) = *guard
            && !h.is_finished()
        {
            log::info!("Aborting CodeGraph watcher...");
            h.abort();
        }
    }

    /// Get the underlying handle for passing to async contexts.
    pub fn inner(&self) -> Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> {
        self.handle.clone()
    }

    /// Get the project path.
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }
}

/// CodeGraph file watcher for auto-sync.
pub struct CodeGraphWatcher {
    project_path: PathBuf,
    stop_tx: broadcast::Sender<()>,
    sync_interval: Duration,
}

impl CodeGraphWatcher {
    /// Check if CodeGraph MCP daemon is already running.
    /// Returns true if daemon is active (skip watcher to avoid conflict).
    pub fn is_daemon_running(project_path: &Path) -> bool {
        // Method 1: Check daemon.pid file
        let daemon_pid_path = project_path.join(".codegraph").join("daemon.pid");
        if daemon_pid_path.exists() {
            let pid_running = std::fs::read_to_string(&daemon_pid_path)
                .ok()
                .and_then(|pid| pid.trim().parse::<u32>().ok())
                .map(|pid| {
                    #[cfg(target_os = "windows")]
                    {
                        use std::os::windows::process::CommandExt;
                        const CREATE_NO_WINDOW: u32 = 0x08000000;
                        std::process::Command::new("tasklist")
                            .args(["/FI", &format!("PID eq {}", pid)])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output()
                            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                            .unwrap_or(false)
                    }
                    #[cfg(not(target_os = "windows"))]
                    std::path::Path::new("/proc").join(pid.to_string()).exists()
                })
                .unwrap_or(false);
            if pid_running {
                return true;
            }
        }

        // Method 2: Check daemon.log for recent activity
        let daemon_log_path = project_path.join(".codegraph").join("daemon.log");
        if daemon_log_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&daemon_log_path) {
                if let Ok(modified) = metadata.modified() {
                    let now = std::time::SystemTime::now();
                    let elapsed = now
                        .duration_since(modified)
                        .unwrap_or(std::time::Duration::MAX);
                    if elapsed < std::time::Duration::from_secs(60) {
                        log::info!("CodeGraph: daemon.log recently modified, daemon likely active");
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Create a new watcher for the project.
    pub fn new(project_path: &Path) -> Self {
        let (stop_tx, _) = broadcast::channel(1);
        Self {
            project_path: project_path.to_path_buf(),
            stop_tx,
            sync_interval: Duration::from_secs(CODEGRAPH_SYNC_INTERVAL_SECS),
        }
    }

    /// Create watcher with automatic project root detection.
    pub fn with_auto_detect(start_path: &Path) -> Self {
        let project_path = find_project_root(start_path);
        log::info!(
            "CodeGraph: detected project root at {}",
            project_path.display()
        );
        Self::new(&project_path)
    }

    /// Start watching for file changes.
    pub fn start(&self, cancel_token: CancellationToken) -> tokio::task::JoinHandle<()> {
        let project_path = self.project_path.clone();
        let sync_interval = self.sync_interval;

        tokio::spawn(async move {
            Self::run_watcher_loop(project_path, sync_interval, cancel_token).await;
        })
    }

    /// Stop the watcher via broadcast signal.
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
            log::warn!(
                "CodeGraph CLI not found, watcher disabled. Please install CodeGraph manually."
            );
            return;
        }

        // Try to acquire watcher lock (prevent multiple instances)
        if !try_acquire_watcher_lock(&project_path) {
            log::info!("CodeGraph: another instance is watching this project, exiting");
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

        // Check if CodeGraph is initialized - DO NOT auto-initialize
        let manager = CodeGraphManager::new(&project_path);
        if !manager.is_initialized() {
            log::info!(
                "CodeGraph: not initialized for {}, skipping watcher. Run 'codegraph init -i' to create index.",
                project_path.display()
            );
            release_watcher_lock(&project_path);
            return;
        }

        // Detect environment type
        let env_type = if is_git_repository(&project_path) {
            CodeGraphEnv::Git
        } else {
            CodeGraphEnv::NonGit
        };

        log::info!(
            "CodeGraph: environment detected as {} for: {}",
            match env_type {
                CodeGraphEnv::Git => "Git repository",
                CodeGraphEnv::NonGit => "non-Git directory",
            },
            project_path.display()
        );

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

        // Channel for file change events
        let (change_tx, mut change_rx) = mpsc::channel::<PathBuf>(100);

        // Create notify file watcher
        let watcher_result = Self::create_file_watcher(&project_path, change_tx.clone());
        if watcher_result.is_err() {
            log::warn!(
                "CodeGraph notify watcher failed to start: {}",
                watcher_result.err().unwrap()
            );
            release_watcher_lock(&project_path);
            return;
        }
        let _watcher = watcher_result.unwrap();

        // Load ignore matcher
        let ignore_matcher = IgnoreMatcher::load(&project_path);

        // Track sync state
        let syncing = Arc::new(AtomicBool::new(false));
        let syncing_clone = syncing.clone();
        let changed_files = Arc::new(RwLock::new(std::collections::HashSet::<PathBuf>::new()));
        let last_change = Arc::new(std::sync::Mutex::new(Instant::now()));

        // Debounce settings
        let debounce_delay = Duration::from_secs(CODEGRAPH_SYNC_INTERVAL_SECS);
        let git_poll_interval = Duration::from_secs(GIT_STATUS_POLL_INTERVAL_SECS);

        // Start Git monitoring if in Git environment
        let git_monitoring = if env_type == CodeGraphEnv::Git {
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
                release_watcher_lock(&project_path);
                log::info!("CodeGraph watcher stopped");
                break;
            }

            // Update heartbeat
            update_watcher_heartbeat(&project_path);

            tokio::select! {
                // Notify file changes
                Some(path) = change_rx.recv() => {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    if is_source_file(&path)
                        && !ignore_matcher.should_ignore(&path, &project_path) {
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

                // Git status polling
                _ = sleep(git_poll_interval), if git_monitoring => {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    let changes = get_git_status_changes(&project_path);
                    if changes.has_changes() {
                        let mut new_count = 0;
                        {
                            let mut files = changed_files.write().await;
                            for path in changes.modified.iter().chain(&changes.added).chain(&changes.deleted) {
                                if files.insert(path.clone()) {
                                    new_count += 1;
                                }
                            }
                            if new_count > 0 {
                                log::debug!(
                                    "CodeGraph [git]: {} new changes (total unique: {})",
                                    new_count,
                                    files.len()
                                );
                            }
                        }
                        if new_count > 0 {
                            *last_change.lock().unwrap() = Instant::now();
                        }
                    }
                }

                // Periodic sync check
                _ = sleep(check_interval) => {
                    if cancel_token.is_cancelled() {
                        break;
                    }

                    let files_count = changed_files.read().await.len();
                    let elapsed = last_change.lock().unwrap().elapsed();

                    if !syncing_clone.load(Ordering::SeqCst)
                        && files_count > 0
                        && elapsed >= debounce_delay {
                        syncing_clone.store(true, Ordering::SeqCst);
                        log::info!("CodeGraph: auto-sync triggered ({} unique files changed)", files_count);

                        // Check if MCP daemon is active before syncing
                        if check_mcp_daemon_active(&project_path) {
                            log::info!("CodeGraph: MCP daemon active, skipping our sync to avoid conflict");
                            syncing_clone.store(false, Ordering::SeqCst);
                        } else {
                            let our_timestamp = try_acquire_sync_lock(&project_path);
                            if our_timestamp > 0 {
                                let manager = CodeGraphManager::new(&project_path);
                                if manager.is_initialized() {
                                    if let Err(e) = manager.sync().await {
                                        log::warn!("CodeGraph sync failed: {}", e);
                                    } else {
                                        // Check if lock still belongs to us before updating
                                        if check_sync_lock_owner(&project_path, our_timestamp) {
                                            update_version_after_sync(&project_path);
                                            changed_files.write().await.clear();
                                            log::debug!("CodeGraph: sync completed, lock verified");
                                        } else {
                                            // Lock was stolen by another process, abandon this sync
                                            log::info!("CodeGraph: sync abandoned, another process took over");
                                            // Don't clear changed_files, let next sync handle them
                                        }
                                    }
                                }
                                release_sync_lock(&project_path);
                            } else {
                                log::debug!("CodeGraph: skipping sync, another instance is syncing");
                            }
                            syncing_clone.store(false, Ordering::SeqCst);
                        }
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
                if !event.kind.is_access() && !event.kind.is_other() {
                    for path in event.paths {
                        let _ = tx.try_send(path);
                    }
                }
            }
        };

        let config = Config::default()
            .with_poll_interval(Duration::from_secs(2))
            .with_compare_contents(false);

        let mut watcher = RecommendedWatcher::new(handler, config)?;
        watcher.watch(project_path, RecursiveMode::Recursive)?;

        Ok(watcher)
    }
}
