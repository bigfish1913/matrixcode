use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search".to_string(),
            description: "在文件中搜索模式，类似 grep 功能".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "要搜索的正则表达式模式"
                    },
                    "path": {
                        "type": "string",
                        "description": "搜索的目录或文件路径（默认 '.'）"
                    },
                    "glob": {
                        "type": "string",
                        "description": "文件过滤的 glob 模式（如 '*.rs'）"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?;
        let path = params["path"].as_str().unwrap_or(".");
        let glob_pattern = params["glob"].as_str();

        // Show spinner while searching - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&msg);

        let pattern = pattern.to_string();
        let path = path.to_string();
        let glob_pattern = glob_pattern.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || search_files(&pattern, &path, glob_pattern.as_deref()))
            .await?
    }
}

fn search_files(pattern: &str, path: &str, glob_pattern: Option<&str>) -> Result<String> {
    use std::fs;
    use std::path::Path;

    let regex = regex::Regex::new(pattern)?;
    let mut results = Vec::new();
    let root = Path::new(path);

    let entries = collect_files(root, glob_pattern)?;

    for file_path in entries {
        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_num, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                results.push(format!(
                    "{}:{}: {}",
                    file_path.display(),
                    line_num + 1,
                    line.trim()
                ));
            }
        }

        if results.len() > 200 {
            results.push("... (truncated, too many results)".to_string());
            break;
        }
    }

    if results.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

fn collect_files(
    root: &std::path::Path,
    glob_pattern: Option<&str>,
) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();

    if root.is_file() {
        files.push(root.to_path_buf());
        return Ok(files);
    }

    let walker = walkdir(root)?;
    let glob_matcher = glob_pattern.map(glob::Pattern::new).transpose()?;

    for entry in walker {
        if let Some(ref matcher) = glob_matcher
            && let Some(name) = entry.file_name().and_then(|n| n.to_str())
            && !matcher.matches(name)
        {
            continue;
        }
        files.push(entry);
    }

    Ok(files)
}

fn walkdir(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    use std::fs;

    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
                continue;
            }

            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}
