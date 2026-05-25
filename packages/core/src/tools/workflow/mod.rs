//! Workflow Tools
//!
//! 工作流相关工具集合

mod discover;
mod run;
mod r#match;
mod content;

pub use discover::WorkflowDiscoverTool;
pub use run::WorkflowRunTool;
pub use r#match::WorkflowMatchTool;
pub use content::ContentGenerationTool;

use crate::tools::BoxedTool;
use std::sync::Arc;
use crate::providers::Provider;

/// Get all workflow management tools
pub fn workflow_tools() -> Vec<BoxedTool> {
    vec![
        Box::new(WorkflowDiscoverTool),
        Box::new(WorkflowRunTool),
        Box::new(WorkflowMatchTool),
    ]
}

/// Get workflow tools that need provider
pub fn workflow_tools_with_provider(provider: Arc<dyn Provider>) -> Vec<BoxedTool> {
    vec![
        Box::new(ContentGenerationTool::new(provider)),
    ]
}