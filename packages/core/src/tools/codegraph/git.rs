//! Git integration for CodeGraph.

use std::path::{Path, PathBuf};

/// Lock file names.
const WATCHER_LOCK_FILE: &str = "watcher.lock";
const SYNC_LOCK_FILE: &str = "sync.lock";
const LOCK_TIMEOUT_SECS: u64 = 30;

/// Version file name for storing Git HEAD SHA.
const VERSION_FILE: &str = "version.txt";

/// Create a command with hidden window on Windows.
#[cfg(windows)]
fn create_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn create_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Check if directory is inside a Git work tree.
pub fn is_git_repository(project_path: &Path) -> bool {
    create_command("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current Git HEAD commit SHA.
pub fn get_git_head_sha(project_path: &Path) -> Option<String> {
    create_command("git")
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
pub fn get_git_tracked_files(project_path: &Path) -> Vec<PathBuf> {
    create_command("git")
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
pub fn get_git_status_changes(project_path: &Path) -> GitStatusChanges {
    let output = create_command("git")
        .args(["status", "--porcelain"])
        .current_dir(project_path)
        .output();

    let mut changes = GitStatusChanges::default();

    if let Ok(o) = output {
        if o.status.success() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if line.len() < 2 {
                    continue;
                }
                let status = &line[..2];
                let path = line[3..].trim();

                let file_path = if path.contains(" -> ") {
                    path.split(" -> ").nth(1).unwrap_or(path)
                } else {
                    path
                };

                let full_path = project_path.join(file_path);

                if !is_source_file(&full_path) {
                    continue;
                }

                match status.trim() {
                    "M" | "MM" | "AM" => changes.modified.push(full_path),
                    "A" | "??" => changes.added.push(full_path),
                    "D" | "AD" | "MD" => changes.deleted.push(full_path),
                    "R" => {
                        if let Some(old_path) = path.split(" -> ").next() {
                            changes.deleted.push(project_path.join(old_path));
                        }
                        changes.added.push(full_path);
                    }
                    _ => {}
                }
            }
        }
    }

    changes
}

/// Start Git fsmonitor daemon (if available).
pub fn start_git_fsmonitor(project_path: &Path) -> bool {
    create_command("git")
        .args(["fsmonitor--daemon", "start"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Git fsmonitor daemon is running.
pub fn is_git_fsmonitor_running(project_path: &Path) -> bool {
    create_command("git")
        .args(["fsmonitor--daemon", "status"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Git status changes result.
#[derive(Debug, Default)]
pub struct GitStatusChanges {
    pub modified: Vec<PathBuf>,
    pub added: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

impl GitStatusChanges {
    pub fn has_changes(&self) -> bool {
        !self.modified.is_empty() || !self.added.is_empty() || !self.deleted.is_empty()
    }

    #[allow(dead_code)]
    pub fn total_count(&self) -> usize {
        self.modified.len() + self.added.len() + self.deleted.len()
    }
}

/// Check if path is a source file based on extension.
pub fn is_source_file(path: &Path) -> bool {
    use super::ignore::WATCH_EXTENSIONS;
    
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| WATCH_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

// ========================================================================
// Lock Management
// ========================================================================

/// Watcher lock to prevent multiple instances.
struct WatcherLock {
    instance_id: String,
    pid: u32,
    acquired_at: i64,
}

impl WatcherLock {
    fn new() -> Self {
        Self {
            instance_id: uuid::Uuid::new_v4().to_string(),
            pid: std::process::id(),
            acquired_at: chrono::Utc::now().timestamp(),
        }
    }

    fn decode(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            instance_id: parts[0].to_string(),
            pid: parts[1].parse().ok()?,
            acquired_at: parts[2].parse().ok()?,
        })
    }

    fn encode(&self) -> String {
        format!("{}|{}|{}", self.instance_id, self.pid, self.acquired_at)
    }

    fn is_stale(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now - self.acquired_at > LOCK_TIMEOUT_SECS as i64
    }
}

/// Try to acquire watcher lock.
pub fn try_acquire_watcher_lock(project_path: &Path) -> bool {
    let lock_path = project_path.join(".codegraph").join(WATCHER_LOCK_FILE);

    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if lock_path.exists() {
        let content = std::fs::read_to_string(&lock_path).ok();
        if let Some(s) = content {
            if let Some(lock) = WatcherLock::decode(&s) {
                if !lock.is_stale() {
                    log::info!(
                        "CodeGraph: watcher lock held by instance {} (PID {}), skipping",
                        lock.instance_id, lock.pid
                    );
                    return false;
                }
                log::info!(
                    "CodeGraph: stealing stale watcher lock from instance {} (PID {})",
                    lock.instance_id, lock.pid
                );
            }
        }
    }

    let lock = WatcherLock::new();
    let _ = std::fs::write(&lock_path, lock.encode());
    log::info!("CodeGraph: acquired watcher lock (instance {})", lock.instance_id);
    true
}

/// Release watcher lock.
pub fn release_watcher_lock(project_path: &Path) {
    let lock_path = project_path.join(".codegraph").join(WATCHER_LOCK_FILE);
    if lock_path.exists() {
        let _ = std::fs::remove_file(&lock_path);
        log::info!("CodeGraph: released watcher lock");
    }
}

/// Update watcher lock heartbeat.
pub fn update_watcher_heartbeat(project_path: &Path) {
    let lock_path = project_path.join(".codegraph").join(WATCHER_LOCK_FILE);
    if lock_path.exists() {
        let lock = WatcherLock::new();
        let _ = std::fs::write(&lock_path, lock.encode());
    }
}

/// Try to acquire sync lock.
pub fn try_acquire_sync_lock(project_path: &Path) -> bool {
    let lock_path = project_path.join(".codegraph").join(SYNC_LOCK_FILE);

    if lock_path.exists() {
        let content = std::fs::read_to_string(&lock_path).ok();
        if let Some(s) = content {
            let timestamp: i64 = s.parse().ok().unwrap_or(0);
            let now = chrono::Utc::now().timestamp();
            if now - timestamp < 5 {
                log::debug!("CodeGraph: sync in progress by another instance, skipping");
                return false;
            }
        }
    }

    let timestamp = chrono::Utc::now().timestamp().to_string();
    let _ = std::fs::write(&lock_path, timestamp);
    true
}

/// Release sync lock.
pub fn release_sync_lock(project_path: &Path) {
    let lock_path = project_path.join(".codegraph").join(SYNC_LOCK_FILE);
    if lock_path.exists() {
        let _ = std::fs::remove_file(&lock_path);
    }
}

// ========================================================================
// Version Tracking
// ========================================================================

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

/// Check if version has changed.
pub fn has_version_changed(project_path: &Path) -> bool {
    let current_sha = get_git_head_sha(project_path);
    let stored_sha = load_version_sha(project_path);
    current_sha != stored_sha
}

/// Update version after successful sync.
pub fn update_version_after_sync(project_path: &Path) {
    if let Some(sha) = get_git_head_sha(project_path) {
        save_version_sha(project_path, &sha);
        log::debug!("CodeGraph: version updated to SHA {}", sha);
    }
}