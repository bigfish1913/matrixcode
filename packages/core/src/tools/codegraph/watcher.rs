//! File watcher for automatic index synchronization.

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::sleep;

use super::manager::CodeGraphManager;
use super::git::{
    is_git_repository, get_git_status_changes, is_git_fsmonitor_running, start_git_fsmonitor,
    has_version_changed, update_version_after_sync,
    try_acquire_watcher_lock, release_watcher_lock,
    try_acquire_sync_lock, release_sync_lock,
    update_watcher_heartbeat,
    is_source_file,
};
use super::ignore::IgnoreMatcher;
use super::install::get_codegraph_path;
use super::project::find_project_root;
use super::types::CodeGraphEnv;
use crate::constants::CODEGRAPH_SYNC_INTERVAL_SECS;
use crate::memory::ProjectStructureAnalyzer;
use crate::cancel::CancellationToken;

/// Git status polling interval (for non-fsmonitor fallback).
const GIT_STATUS_POLL_INTERVAL_SECS: u64 = 2;

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
            sync_interval: Duration::from_secs(CODEGRAPH_SYNC_INTERVAL_SECS),
        }
    }

    /// Create watcher with automatic project root detection.
    pub fn with_auto_detect(start_path: &Path) -> Self {
        let project_path = find_project_root(start_path);
        log::info!("CodeGraph: detected project root at {}", project_path.display());
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
            log::warn!("CodeGraph CLI not found, watcher disabled. Please install CodeGraph manually.");
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
            log::warn!("CodeGraph notify watcher failed to start: {}", watcher_result.err().unwrap());
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

                        if try_acquire_sync_lock(&project_path) {
                            let manager = CodeGraphManager::new(&project_path);
                            if manager.is_initialized() {
                                if let Err(e) = manager.sync().await {
                                    log::warn!("CodeGraph sync failed: {}", e);
                                } else {
                                    update_version_after_sync(&project_path);
                                }
                                changed_files.write().await.clear();
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