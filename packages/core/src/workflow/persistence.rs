//! Workflow persistence module
//!
//! Provides save/load functionality for workflow contexts
//!
//! Supports two storage locations:
//! - User directory: `~/.matrix/workflows/` (global workflows)
//! - Project directory: `.matrix/workflows/` (project-specific workflows)

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use super::context::WorkflowContext;

/// Workflow persistence manager
///
/// Handles saving and loading workflow contexts to/from disk.
/// Files are stored in JSON format under:
/// - `~/.matrix/workflows/` (user/global)
/// - `.matrix/workflows/` (project-specific)
pub struct WorkflowPersistence {
    /// User directory for global workflows
    user_path: PathBuf,
    /// Project directory for project-specific workflows (optional)
    project_path: Option<PathBuf>,
    /// Preferred save location (project if available, else user)
    save_path: PathBuf,
}

impl WorkflowPersistence {
    /// Create a new persistence manager with project context
    ///
    /// Searches both project and user directories when loading.
    /// Saves to project directory if available, else to user directory.
    pub fn new(project_dir: Option<&PathBuf>) -> Self {
        // User directory: ~/.matrix/workflows/
        let user_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".matrix")
            .join("workflows");

        // Project directory: <project>/ .matrix/workflows/
        let project_path = project_dir.map(|p| p.join(".matrix").join("workflows"));

        // Save path: prefer project if available
        let save_path = project_path
            .as_ref()
            .filter(|_p| {
                // Only use project path if project directory exists
                project_dir.as_ref().map(|pd| pd.exists()).unwrap_or(false)
            })
            .cloned()
            .unwrap_or_else(|| user_path.clone());

        // Ensure directories exist
        if let Err(e) = fs::create_dir_all(&user_path) {
            log::warn!("Failed to create user workflow directory: {}", e);
        }
        if let Some(ref proj) = project_path
            && let Err(e) = fs::create_dir_all(proj)
        {
            log::warn!("Failed to create project workflow directory: {}", e);
        }

        Self {
            user_path,
            project_path,
            save_path,
        }
    }

    /// Create a persistence manager for user directory only (no project context)
    pub fn new_global() -> Self {
        Self::new(None)
    }

    /// Create a persistence manager with a custom base path
    ///
    /// Useful for testing or custom configurations.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        if let Err(e) = fs::create_dir_all(&base_path) {
            log::warn!("Failed to create workflow directory: {}", e);
        }
        Self {
            user_path: base_path.clone(),
            project_path: None,
            save_path: base_path,
        }
    }

    /// Get the file path for saving a workflow run
    fn get_save_file_path(&self, instance_id: &str) -> PathBuf {
        self.save_path.join(format!("{}.json", instance_id))
    }

    /// Search for a workflow file in both project and user directories
    fn find_file_path(&self, instance_id: &str) -> Option<PathBuf> {
        // Search project first, then user
        if let Some(ref proj) = self.project_path {
            let proj_path = proj.join(format!("{}.json", instance_id));
            if proj_path.exists() {
                return Some(proj_path);
            }
        }

        let user_path = self.user_path.join(format!("{}.json", instance_id));
        if user_path.exists() {
            return Some(user_path);
        }

        None
    }

    /// Save a workflow context to disk
    ///
    /// The context is serialized to JSON and stored in the save directory
    /// (project directory if available, else user directory).
    ///
    /// # Example
    /// ```ignore
    /// let persistence = WorkflowPersistence::new(Some(&project_path));
    /// let ctx = WorkflowContext::new("my_workflow".to_string(), HashMap::new());
    /// persistence.save(&ctx)?;
    /// ```
    pub fn save(&self, context: &WorkflowContext) -> Result<()> {
        let path = self.get_save_file_path(&context.instance_id);
        let content = serde_json::to_string_pretty(context)?;

        fs::write(&path, content)?;
        log::info!("Saved workflow context to {:?}", path);

        Ok(())
    }

    /// Load a workflow context from disk
    ///
    /// Searches both project and user directories.
    /// Returns `Ok(None)` if the instance_id does not exist.
    ///
    /// # Example
    /// ```ignore
    /// let persistence = WorkflowPersistence::new(Some(&project_path));
    /// if let Some(ctx) = persistence.load("run-123")? {
    ///     println!("Loaded workflow: {}", ctx.workflow_id);
    /// }
    /// ```
    pub fn load(&self, instance_id: &str) -> Result<Option<WorkflowContext>> {
        let path = self.find_file_path(instance_id);

        if let Some(path) = path {
            let content = fs::read_to_string(&path)?;
            let context: WorkflowContext = serde_json::from_str(&content)?;
            log::info!("Loaded workflow context from {:?}", path);
            Ok(Some(context))
        } else {
            Ok(None)
        }
    }

    /// List all saved workflow contexts from both directories
    ///
    /// Returns a list of all persisted workflow contexts.
    /// Failed loads are logged and skipped.
    ///
    /// # Example
    /// ```ignore
    /// let persistence = WorkflowPersistence::new(Some(&project_path));
    /// let workflows = persistence.list()?;
    /// for ctx in workflows {
    ///     println!("{}: {:?}", ctx.instance_id, ctx.status);
    /// }
    /// ```
    pub fn list(&self) -> Result<Vec<WorkflowContext>> {
        let mut contexts = Vec::new();

        // Helper to load contexts from a directory
        fn load_from_dir(dir: &PathBuf, contexts: &mut Vec<WorkflowContext>) -> Result<()> {
            if !dir.exists() {
                return Ok(());
            }

            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().is_some_and(|ext| ext == "json") {
                    match fs::read_to_string(&path) {
                        Ok(content) => match serde_json::from_str::<WorkflowContext>(&content) {
                            Ok(ctx) => contexts.push(ctx),
                            Err(e) => {
                                log::warn!("Failed to parse {:?}: {}", path, e);
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to read {:?}: {}", path, e);
                        }
                    }
                }
            }
            Ok(())
        }

        // Load from both directories
        if let Some(ref proj) = self.project_path {
            load_from_dir(proj, &mut contexts)?;
        }
        load_from_dir(&self.user_path, &mut contexts)?;

        // Sort by created_at descending (newest first)
        contexts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(contexts)
    }

    /// Delete a saved workflow context
    ///
    /// Searches both directories and deletes from where it exists.
    /// Returns `Ok(())` even if the file does not exist.
    ///
    /// # Example
    /// ```ignore
    /// let persistence = WorkflowPersistence::new(Some(&project_path));
    /// persistence.delete("run-123")?;
    /// ```
    pub fn delete(&self, instance_id: &str) -> Result<()> {
        if let Some(path) = self.find_file_path(instance_id) {
            fs::remove_file(&path)?;
            log::info!("Deleted workflow context: {:?}", path);
        }

        Ok(())
    }

    /// List workflow contexts by status
    ///
    /// # Example
    /// ```ignore
    /// let persistence = WorkflowPersistence::new(Some(&project_path));
    /// let paused = persistence.list_by_status(WorkflowStatus::Paused)?;
    /// ```
    pub fn list_by_status(
        &self,
        status: super::context::WorkflowStatus,
    ) -> Result<Vec<WorkflowContext>> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|ctx| ctx.status == status).collect())
    }

    /// Get the save path (where new workflows will be saved)
    pub fn save_path(&self) -> &PathBuf {
        &self.save_path
    }

    /// Get the user path
    pub fn user_path(&self) -> &PathBuf {
        &self.user_path
    }

    /// Get the project path (if available)
    pub fn project_path(&self) -> Option<&PathBuf> {
        self.project_path.as_ref()
    }
}

impl Default for WorkflowPersistence {
    fn default() -> Self {
        Self::new_global()
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::WorkflowStatus;
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        let mut ctx = WorkflowContext::new("test_workflow".to_string(), HashMap::new());
        ctx.start();

        // Save
        persistence.save(&ctx).unwrap();

        // Load
        let loaded = persistence.load(&ctx.instance_id).unwrap();
        assert!(loaded.is_some());

        let loaded_ctx = loaded.unwrap();
        assert_eq!(loaded_ctx.instance_id, ctx.instance_id);
        assert_eq!(loaded_ctx.workflow_id, "test_workflow");
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        let loaded = persistence.load("nonexistent-id").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_list() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        // Create multiple contexts
        let mut ctx1 = WorkflowContext::new("workflow1".to_string(), HashMap::new());
        ctx1.start();
        let mut ctx2 = WorkflowContext::new("workflow2".to_string(), HashMap::new());
        ctx2.start();

        persistence.save(&ctx1).unwrap();
        persistence.save(&ctx2).unwrap();

        let list = persistence.list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_delete() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        let ctx = WorkflowContext::new("test".to_string(), HashMap::new());
        persistence.save(&ctx).unwrap();

        // Verify exists
        assert!(persistence.load(&ctx.instance_id).unwrap().is_some());

        // Delete
        persistence.delete(&ctx.instance_id).unwrap();

        // Verify deleted
        assert!(persistence.load(&ctx.instance_id).unwrap().is_none());
    }

    #[test]
    fn test_save_and_load_paused_workflow() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        let mut ctx = WorkflowContext::new("paused_workflow".to_string(), HashMap::new());
        ctx.start();
        ctx.set_current_node("node1".to_string());
        ctx.pause();

        // Save paused workflow
        persistence.save(&ctx).unwrap();

        // Load and verify status
        let loaded = persistence.load(&ctx.instance_id).unwrap().unwrap();
        assert_eq!(loaded.status, WorkflowStatus::Paused);
        assert_eq!(loaded.current_node_id, Some("node1".to_string()));

        // Resume and save again
        let mut loaded = loaded;
        loaded.resume();
        persistence.save(&loaded).unwrap();

        // Verify resumed status
        let resumed = persistence.load(&loaded.instance_id).unwrap().unwrap();
        assert_eq!(resumed.status, WorkflowStatus::Running);
    }

    #[test]
    fn test_list_by_status() {
        let dir = tempdir().unwrap();
        let persistence = WorkflowPersistence::with_base_path(dir.path().to_path_buf());

        // Create workflows with different statuses
        let mut ctx1 = WorkflowContext::new("workflow1".to_string(), HashMap::new());
        ctx1.start();
        ctx1.pause();

        let mut ctx2 = WorkflowContext::new("workflow2".to_string(), HashMap::new());
        ctx2.start();

        persistence.save(&ctx1).unwrap();
        persistence.save(&ctx2).unwrap();

        // List paused workflows
        let paused = persistence.list_by_status(WorkflowStatus::Paused).unwrap();
        assert_eq!(paused.len(), 1);
        assert_eq!(paused[0].workflow_id, "workflow1");

        // List running workflows
        let running = persistence.list_by_status(WorkflowStatus::Running).unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].workflow_id, "workflow2");
    }
}
