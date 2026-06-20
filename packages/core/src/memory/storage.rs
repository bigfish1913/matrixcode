//! Memory storage with file locking.

use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::constants::MATRIX_DIR;
use super::config::MemoryConfig;
use super::entry::MemoryEntry;
use super::manager::AutoMemory;

// ============================================================================
// File Lock
// ============================================================================

/// File lock for preventing concurrent access to memory storage.
pub struct MemoryFileLock {
    /// Path to the lock file.
    lock_path: PathBuf,
    /// Whether we currently hold the lock.
    locked: bool,
}

impl MemoryFileLock {
    /// Create a new file lock for the given directory.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            lock_path: base_dir.join("memory.lock"),
            locked: false,
        }
    }

    /// Acquire the lock (blocking with timeout).
    /// Returns Ok(true) if lock acquired, Err if timeout.
    pub fn acquire(&mut self, timeout_ms: u64) -> Result<()> {
        if self.locked {
            return Ok(());
        }

        let start = std::time::Instant::now();

        while start.elapsed().as_millis() < timeout_ms as u128 {
            match fs::File::create_new(&self.lock_path) {
                Ok(_) => {
                    let lock_info = format!("{}:{}", std::process::id(), Utc::now().to_rfc3339());
                    fs::write(&self.lock_path, lock_info)?;
                    self.locked = true;
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if self.is_stale_lock()? {
                        self.remove_stale_lock()?;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        // Timeout - return error instead of Ok(false)
        anyhow::bail!("Failed to acquire memory lock after {}ms timeout", timeout_ms)
    }

    /// Check if the existing lock is stale (either old or process is dead).
    fn is_stale_lock(&self) -> Result<bool> {
        if !self.lock_path.exists() {
            return Ok(false);
        }

        // First check if the lock owner process is still running
        if let Ok(content) = fs::read_to_string(&self.lock_path)
            && let Some(pid_str) = content.split(':').next()
            && let Ok(pid) = pid_str.parse::<u32>()
            && !self.is_process_running(pid)
        {
            // Process is dead, lock is stale
            return Ok(true);
        }

        // Then check lock age as fallback
        let metadata = fs::metadata(&self.lock_path)?;
        let modified = metadata.modified()?;
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or(std::time::Duration::ZERO);

        Ok(age > std::time::Duration::from_secs(60))
    }

    /// Check if a process with the given PID is still running.
    fn is_process_running(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix, check if process exists by checking /proc
            if std::path::Path::new("/proc").exists() {
                std::path::Path::new(&format!("/proc/{}", pid)).exists()
            } else {
                // Fallback: assume process is running if we can't check
                true
            }
        }
        #[cfg(windows)]
        {
            // On Windows, use tasklist command to check if process exists
            use std::process::Command;
            let output = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output();

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    // tasklist returns "INFO: No tasks are running..." if process not found
                    // or returns a line with the PID if running
                    stdout.contains(&pid.to_string()) && !stdout.contains("No tasks")
                }
                Err(_) => {
                    // If tasklist fails, assume process might be running (safer)
                    true
                }
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = pid;
            true
        }
    }

    /// Remove stale lock file with atomic retry.
    fn remove_stale_lock(&self) -> Result<()> {
        // Use atomic rename to avoid race condition
        // Create a temp deletion marker and rename it over the lock
        let temp_path = self.lock_path.with_extension("lock.del");
        if self.lock_path.exists() {
            // Try atomic rename first
            if fs::rename(&self.lock_path, &temp_path).is_ok() {
                fs::remove_file(&temp_path)?;
            } else {
                // Fallback to direct removal if rename fails
                fs::remove_file(&self.lock_path)?;
            }
        }
        Ok(())
    }

    /// Release the lock.
    pub fn release(&mut self) -> Result<()> {
        if self.locked {
            fs::remove_file(&self.lock_path)?;
            self.locked = false;
        }
        Ok(())
    }
}

impl Drop for MemoryFileLock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

// ============================================================================
// Memory Storage
// ============================================================================

/// Storage for memory files (global and project-level) with file locking.
pub struct MemoryStorage {
    /// Base directory for global memory (~/.matrix).
    base_dir: PathBuf,
    /// Project root directory (optional).
    project_root: Option<PathBuf>,
    /// File lock for preventing concurrent writes.
    lock: MemoryFileLock,
}

impl MemoryStorage {
    /// Create a new memory storage.
    pub fn new(project_root: Option<&Path>) -> Result<Self> {
        let base_dir = Self::get_base_dir()?;
        let lock = MemoryFileLock::new(&base_dir);
        Ok(Self {
            base_dir,
            project_root: project_root.map(|p| p.to_path_buf()),
            lock,
        })
    }

    /// Create a new storage with explicit lock timeout.
    pub fn with_lock_timeout(project_root: Option<&Path>, timeout_ms: u64) -> Result<Self> {
        let mut storage = Self::new(project_root)?;
        storage.lock.acquire(timeout_ms)?;
        Ok(storage)
    }

    /// Get the base directory for memory storage.
    fn get_base_dir() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE not set"))?;
        let mut p = PathBuf::from(home);
        p.push(MATRIX_DIR);
        Ok(p)
    }

    /// Path to global memory file.
    pub fn global_memory_path(&self) -> PathBuf {
        self.base_dir.join("memory.json")
    }

    /// Path to project memory file.
    pub fn project_memory_path(&self) -> Option<PathBuf> {
        self.project_root
            .as_ref()
            .map(|p| p.join(".matrix/memory.json"))
    }

    /// Path to config file.
    pub fn config_path(&self) -> PathBuf {
        self.base_dir.join("memory_config.json")
    }

    /// Ensure directories exist.
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        if let Some(root) = &self.project_root {
            let memory_dir = root.join(MATRIX_DIR);
            fs::create_dir_all(memory_dir)?;
        }
        Ok(())
    }

    /// Acquire lock before write operations.
    fn acquire_lock(&mut self) -> Result<()> {
        self.lock.acquire(5000)?;
        Ok(())
    }

    /// Release lock after write operations.
    fn release_lock(&mut self) -> Result<()> {
        self.lock.release()?;
        Ok(())
    }

    /// Load global memory.
    pub fn load_global(&self) -> Result<AutoMemory> {
        let path = self.global_memory_path();
        if !path.exists() {
            return Ok(AutoMemory::new());
        }
        let data = fs::read_to_string(&path)?;
        let memory: AutoMemory = serde_json::from_str(&data)?;
        Ok(memory)
    }

    /// Load project memory.
    pub fn load_project(&self) -> Result<Option<AutoMemory>> {
        let path = self.project_memory_path();
        match path {
            Some(p) if p.exists() => {
                let data = fs::read_to_string(&p)?;
                let memory: AutoMemory = serde_json::from_str(&data)?;
                Ok(Some(memory))
            }
            _ => Ok(None),
        }
    }

    /// Load combined memory (global + project).
    pub fn load_combined(&self) -> Result<AutoMemory> {
        let mut combined = self.load_global()?;

        if let Some(project) = self.load_project()? {
            for entry in project.entries {
                let mut tagged_entry = entry;
                if !tagged_entry.tags.contains(&"project".to_string()) {
                    tagged_entry.tags.push("project".to_string());
                }
                combined.entries.push(tagged_entry);
            }
            combined.prune();
        }

        Ok(combined)
    }

    /// Save global memory (with file lock).
    pub fn save_global(&mut self, memory: &AutoMemory) -> Result<()> {
        self.acquire_lock()?;
        self.ensure_dirs()?;

        let path = self.global_memory_path();
        let json = serde_json::to_string_pretty(memory)?;

        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;

        self.release_lock()?;
        Ok(())
    }

    /// Save project memory (with file lock).
    pub fn save_project(&mut self, memory: &AutoMemory) -> Result<()> {
        self.acquire_lock()?;
        self.ensure_dirs()?;

        let path = self
            .project_memory_path()
            .ok_or_else(|| anyhow::anyhow!("no project root"))?;
        let json = serde_json::to_string_pretty(memory)?;

        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;

        self.release_lock()?;
        Ok(())
    }

    /// Save config to separate file.
    pub fn save_config(&mut self, config: &MemoryConfig) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.config_path();
        let json = serde_json::to_string_pretty(config)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Load config from file.
    pub fn load_config(&self) -> Result<MemoryConfig> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(MemoryConfig::default());
        }
        let data = fs::read_to_string(&path)?;
        let config: MemoryConfig = serde_json::from_str(&data)?;
        Ok(config)
    }

    /// Add entry to appropriate storage.
    pub fn add_entry(&mut self, entry: MemoryEntry, is_project_specific: bool) -> Result<()> {
        self.acquire_lock()?;

        if is_project_specific {
            let mut project = self.load_project()?.unwrap_or_else(AutoMemory::new);
            project.add(entry);
            self.save_project_locked(&project)?;
        } else {
            let mut global = self.load_global()?;
            global.add(entry);
            self.save_global_locked(&global)?;
        }

        self.release_lock()?;
        Ok(())
    }

    /// Remove entry from storage by ID.
    pub fn remove_entry(&mut self, id: &str, is_project_specific: bool) -> Result<bool> {
        self.acquire_lock()?;

        let removed = if is_project_specific {
            if let Some(mut project) = self.load_project()? {
                let removed = project.remove(id);
                if removed {
                    self.save_project_locked(&project)?;
                }
                removed
            } else {
                false
            }
        } else {
            let mut global = self.load_global()?;
            let removed = global.remove(id);
            if removed {
                self.save_global_locked(&global)?;
            }
            removed
        };

        self.release_lock()?;
        Ok(removed)
    }

    /// Internal save methods (assumed already locked).
    fn save_global_locked(&self, memory: &AutoMemory) -> Result<()> {
        let path = self.global_memory_path();
        let json = serde_json::to_string_pretty(memory)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn save_project_locked(&self, memory: &AutoMemory) -> Result<()> {
        let path = self
            .project_memory_path()
            .ok_or_else(|| anyhow::anyhow!("no project root"))?;
        let json = serde_json::to_string_pretty(memory)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}
