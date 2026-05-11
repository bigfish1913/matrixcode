pub mod edit;
pub mod read;
pub mod search;
pub mod webfetch;
pub mod write;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

pub fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(read::ReadTool),
        Box::new(write::WriteTool),
        Box::new(edit::EditTool),
        Box::new(search::SearchTool),
        Box::new(webfetch::WebFetchTool),
    ]
}
