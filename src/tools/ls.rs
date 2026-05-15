use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use super::spinner::ToolSpinner;

pub struct LsTool;

#[async_trait]
impl Tool for LsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ls".to_string(),
            description:
                "List immediate entries of a directory. Returns one entry per line, \
                 dirs first (suffixed with /), then files with byte size. Does not \
                 recurse — use 'glob' for recursive discovery."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (defaults to '.')"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"].as_str().unwrap_or(".").to_string();

        // Show spinner while listing - RAII guard ensures cleanup on error
        let spinner = ToolSpinner::new(&format!("listing {}", path));

        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<(String, u64)> = Vec::new();

        let mut rd = tokio::fs::read_dir(&path).await?;
        while let Some(entry) = rd.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                dirs.push(format!("{}/", name));
            } else {
                let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                files.push((name, size));
            }
        }

        dirs.sort();
        files.sort_by(|a, b| a.0.cmp(&b.0));

        let total = dirs.len() + files.len();

        if dirs.is_empty() && files.is_empty() {
            spinner.finish_success("(empty)");
            return Ok(format!("(empty) {}", path));
        }

        let mut out = String::new();
        for d in dirs {
            out.push_str(&d);
            out.push('\n');
        }
        for (n, s) in files {
            out.push_str(&format!("{}  ({} B)\n", n, s));
        }
        while out.ends_with('\n') {
            out.pop();
        }

        spinner.finish_success(&format!("{} entries", total));
        Ok(out)
    }
}