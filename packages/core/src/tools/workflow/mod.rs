//! Workflow Tools
//!
//! 工作流相关工具集合

mod content;
mod create;
mod discover;
mod r#match;
mod run;

pub use content::ContentGenerationTool;
pub use create::WorkflowCreateTool;
pub use discover::WorkflowDiscoverTool;
pub use r#match::WorkflowMatchTool;
pub use run::WorkflowRunTool;

use crate::providers::Provider;
use crate::tools::BoxedTool;
use std::sync::Arc;

/// Get all workflow management tools
pub fn workflow_tools() -> Vec<BoxedTool> {
    vec![
        Box::new(WorkflowDiscoverTool),
        Box::new(WorkflowRunTool::new()),
        Box::new(WorkflowMatchTool),
        Box::new(WorkflowCreateTool),
    ]
}

/// Get workflow tools that need provider
pub fn workflow_tools_with_provider(provider: Arc<dyn Provider>) -> Vec<BoxedTool> {
    vec![
        Box::new(WorkflowRunTool::with_provider(provider.clone())),
        Box::new(ContentGenerationTool::new(provider)),
    ]
}
