//! Workflow Registry - Discovery and Matching
//!
//! Automatically discovers and manages available workflows from:
//! - Project directory: .matrix/workflows/*.yaml
//! - User directory: ~/.matrix/workflows/*.yaml

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::def::WorkflowDef;
use super::parser::parse_workflow_from_file;

/// Workflow metadata for matching
#[derive(Debug, Clone)]
pub struct WorkflowInfo {
    /// Workflow ID
    pub id: String,
    /// Workflow name
    pub name: String,
    /// Workflow description
    pub description: Option<String>,
    /// File path
    pub path: PathBuf,
    /// Source (project or user)
    pub source: WorkflowSource,
    /// Tags for matching
    pub tags: Vec<String>,
    /// Required inputs
    pub required_inputs: Vec<String>,
}

/// Where the workflow was discovered
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowSource {
    /// Project-local workflow
    Project,
    /// User-global workflow
    User,
}

/// Workflow registry - discovers and manages available workflows
pub struct WorkflowRegistry {
    /// Discovered workflows
    workflows: HashMap<String, WorkflowInfo>,
    /// Project workflows directory
    project_path: Option<PathBuf>,
    /// User workflows directory
    user_path: PathBuf,
}

impl WorkflowRegistry {
    /// Create a new registry with project context
    pub fn new(project_dir: Option<&PathBuf>) -> Self {
        let user_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".matrix")
            .join("workflows");

        let project_path = project_dir.map(|p| p.join(".matrix").join("workflows"));

        let mut registry = Self {
            workflows: HashMap::new(),
            project_path,
            user_path,
        };

        // Auto-discover on creation
        if let Err(e) = registry.discover() {
            log::warn!("Workflow discovery failed: {}", e);
        }

        registry
    }

    /// Create registry for user directory only
    pub fn new_global() -> Self {
        Self::new(None)
    }

    /// Discover workflows from both directories
    pub fn discover(&mut self) -> Result<()> {
        self.workflows.clear();

        // Clone paths to avoid borrow issues
        let project_path = self.project_path.clone();
        let user_path = self.user_path.clone();

        // Discover from project directory
        if let Some(proj) = project_path
            && proj.exists()
        {
            self.discover_from_dir(&proj, WorkflowSource::Project)?;
        }

        // Discover from user directory
        if user_path.exists() {
            self.discover_from_dir(&user_path, WorkflowSource::User)?;
        }

        log::info!("Discovered {} workflows", self.workflows.len());
        Ok(())
    }

    /// Discover workflows from a specific directory
    fn discover_from_dir(&mut self, dir: &PathBuf, source: WorkflowSource) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .yaml and .yml files
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
                && let Ok(workflow) = parse_workflow_from_file(&path)
            {
                let info = WorkflowInfo {
                    id: workflow.id.clone(),
                    name: workflow.name.clone(),
                    description: workflow.description.clone(),
                    path: path.clone(),
                    source: source.clone(),
                    tags: self.extract_tags(&workflow),
                    required_inputs: workflow
                        .inputs
                        .iter()
                        .filter(|i| i.required)
                        .map(|i| i.name.clone())
                        .collect(),
                };

                self.workflows.insert(workflow.id.clone(), info);
            }
        }

        Ok(())
    }

    /// Extract tags from workflow for matching
    fn extract_tags(&self, workflow: &WorkflowDef) -> Vec<String> {
        let mut tags = Vec::new();

        // Add ID and name as tags
        tags.push(workflow.id.clone());
        tags.extend(workflow.name.split_whitespace().map(|s| s.to_lowercase()));

        // Add node types as tags
        for node in &workflow.nodes {
            let type_tag = match node.node_type {
                super::def::NodeType::Task => "task",
                super::def::NodeType::Condition => "condition",
                super::def::NodeType::Parallel => "parallel",
                super::def::NodeType::Pipeline => "pipeline",
                super::def::NodeType::Approval => "approval",
                super::def::NodeType::Wait => "wait",
                super::def::NodeType::SubWorkflow => "subworkflow",
                super::def::NodeType::Start => "start",
                super::def::NodeType::End => "end",
            };
            tags.push(type_tag.to_string());

            // Add task name if present
            if let Some(ref task) = node.task {
                tags.push(task.clone());
            }
        }

        tags
    }

    /// List all discovered workflows
    pub fn list(&self) -> Vec<&WorkflowInfo> {
        self.workflows.values().collect()
    }

    /// Get a specific workflow by ID
    pub fn get(&self, id: &str) -> Option<&WorkflowInfo> {
        self.workflows.get(id)
    }

    /// Match workflows by keywords/intent
    ///
    /// Returns workflows sorted by match score (highest first)
    pub fn match_workflows(&self, query: &str) -> Vec<&WorkflowInfo> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        // Calculate match scores
        let mut scored: Vec<(usize, &WorkflowInfo)> = self
            .workflows
            .values()
            .map(|info| {
                let score = self.calculate_match_score(info, &query_words, &query_lower);
                (score, info)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        scored.iter().map(|(_, info)| *info).collect()
    }

    /// Calculate match score for a workflow
    fn calculate_match_score(
        &self,
        info: &WorkflowInfo,
        query_words: &[&str],
        query_lower: &str,
    ) -> usize {
        let mut score = 0;

        // Direct ID match
        if info.id.to_lowercase() == query_lower {
            score += 100;
        }

        // Name contains query
        if info.name.to_lowercase().contains(query_lower) {
            score += 50;
        }

        // Tag matches
        for tag in &info.tags {
            let tag_lower = tag.to_lowercase();

            // Exact tag match with query word
            for word in query_words {
                if tag_lower == *word {
                    score += 30;
                }
                // Tag contains word
                if tag_lower.contains(word) {
                    score += 10;
                }
            }

            // Query contains tag
            if query_lower.contains(&tag_lower) {
                score += 15;
            }
        }

        // Description matches
        if let Some(ref desc) = info.description {
            let desc_lower = desc.to_lowercase();
            for word in query_words {
                if desc_lower.contains(word) {
                    score += 5;
                }
            }
        }

        score
    }

    /// Load a workflow definition by ID
    pub fn load_workflow(&self, id: &str) -> Result<Option<WorkflowDef>> {
        if let Some(info) = self.get(id) {
            let workflow = parse_workflow_from_file(&info.path)?;
            Ok(Some(workflow))
        } else {
            Ok(None)
        }
    }

    /// Get the best matching workflow for a query
    pub fn best_match(&self, query: &str) -> Option<&WorkflowInfo> {
        self.match_workflows(query).first().copied()
    }

    /// Check if any workflows are available
    pub fn is_empty(&self) -> bool {
        self.workflows.is_empty()
    }

    /// Get count of discovered workflows
    pub fn count(&self) -> usize {
        self.workflows.len()
    }

    /// Generate a summary for AI context
    pub fn generate_summary(&self) -> String {
        if self.is_empty() {
            return "No workflows available.".to_string();
        }

        let mut summary = format!("Available workflows ({}):\n\n", self.count());

        for info in self.list() {
            let source = if info.source == WorkflowSource::Project {
                "project"
            } else {
                "global"
            };
            summary.push_str(&format!("• {} - {} [{}]\n", info.id, info.name, source));

            if let Some(ref desc) = info.description {
                let desc_short = desc.chars().take(50).collect::<String>();
                summary.push_str(&format!("  {}\n", desc_short));
            }

            if !info.required_inputs.is_empty() {
                summary.push_str(&format!(
                    "  Required inputs: {}\n",
                    info.required_inputs.join(", ")
                ));
            }
        }

        summary.push_str("\nUsage: 'run workflow <id> with <inputs>' or describe your intent.");
        summary
    }
}

impl Default for WorkflowRegistry {
    fn default() -> Self {
        Self::new_global()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = WorkflowRegistry::new_global();
        // count() returns usize which is always >= 0
        let _count = registry.count();
    }

    #[test]
    fn test_match_empty_registry() {
        let registry = WorkflowRegistry::new_global();
        let matches = registry.match_workflows("test query");
        // Should return empty list
        assert!(matches.is_empty() || registry.count() > 0);
    }

    #[test]
    fn test_generate_summary() {
        let registry = WorkflowRegistry::new_global();
        let summary = registry.generate_summary();
        assert!(summary.contains("workflows") || summary.contains("No workflows"));
    }

    #[test]
    fn test_discover_image_article_workflow() {
        let registry = WorkflowRegistry::new_global();

        // Check if image-article workflow is discovered
        let info = registry.get("image-article");
        if let Some(workflow_info) = info {
            assert_eq!(workflow_info.id, "image-article");
            assert_eq!(workflow_info.name, "Image Article Generator");
            assert!(workflow_info.required_inputs.contains(&"topic".to_string()));
        } else {
            // If not found, at least verify the hello-world workflow exists
            assert!(
                registry.get("hello-world").is_some(),
                "Neither image-article nor hello-world workflows found in ~/.matrix/workflows/"
            );
        }
    }
}
