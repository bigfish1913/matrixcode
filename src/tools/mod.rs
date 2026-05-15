pub mod bash;
pub mod edit;
pub mod glob;
pub mod ls;
pub mod multi_edit;
pub mod read;
pub mod search;
pub mod skill;
pub mod spinner;
pub mod todo_write;
pub mod webfetch;
pub mod websearch;
pub mod write;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::skills::Skill;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, params: Value) -> Result<String>;
}

/// Default toolset without any skill integration. Kept for callers
/// (and the existing tests) that don't care about skills.
pub fn all_tools() -> Vec<Box<dyn Tool>> {
    all_tools_with_skills(Arc::new(Vec::new()))
}

/// Build the toolset and include the `skill` tool bound to the given
/// skills catalogue. The catalogue can be empty; the tool still works
/// but will only report "no skills loaded" if invoked.
pub fn all_tools_with_skills(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(read::ReadTool),
        Box::new(write::WriteTool),
        Box::new(edit::EditTool),
        Box::new(multi_edit::MultiEditTool),
        Box::new(search::SearchTool),
        Box::new(glob::GlobTool),
        Box::new(ls::LsTool),
        Box::new(bash::BashTool),
        Box::new(todo_write::TodoWriteTool),
        Box::new(websearch::WebSearchTool),
        Box::new(webfetch::WebFetchTool),
        Box::new(skill::SkillTool::new(skills)),
    ]
}