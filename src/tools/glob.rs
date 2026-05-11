use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct GlobTool;

const MAX_RESULTS: usize = 200;

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description:
                "Find files by glob pattern (e.g. '**/*.rs', 'src/*.toml'). \
                 Returns matching file paths sorted by modification time (newest first). \
                 Use this to locate files by name; use 'search' to find content inside files."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern, supports '*', '?', and recursive '**'"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory to search in (defaults to '.')"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?
            .to_string();
        let path = params["path"].as_str().unwrap_or(".").to_string();

        tokio::task::spawn_blocking(move || find_files(&pattern, &path)).await?
    }
}

fn find_files(pattern: &str, path: &str) -> Result<String> {
    let full_pattern = if path.is_empty() || path == "." {
        pattern.to_string()
    } else {
        format!("{}/{}", path.trim_end_matches('/'), pattern)
    };

    let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in glob::glob(&full_pattern)? {
        let p = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };
        if should_skip(&p) || !p.is_file() {
            continue;
        }
        let mtime = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        matches.push((p, mtime));

        if matches.len() > MAX_RESULTS * 2 {
            break;
        }
    }

    matches.sort_by(|a, b| b.1.cmp(&a.1));

    let total = matches.len();
    let truncated = total > MAX_RESULTS;
    matches.truncate(MAX_RESULTS);

    if matches.is_empty() {
        return Ok("No files matched.".to_string());
    }

    let mut out = matches
        .into_iter()
        .map(|(p, _)| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    if truncated {
        out.push_str(&format!(
            "\n... (showing {} of {}+ matches)",
            MAX_RESULTS, total
        ));
    }

    Ok(out)
}

fn should_skip(p: &Path) -> bool {
    const IGNORED: &[&str] = &[".git", ".hg", ".svn", "node_modules", "target"];
    for c in p.components() {
        if let std::path::Component::Normal(s) = c {
            if let Some(name) = s.to_str() {
                if IGNORED.contains(&name) {
                    return true;
                }
            }
        }
    }
    false
}
