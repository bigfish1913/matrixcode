use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolDefinition};

pub struct GlobTool;

const MAX_RESULTS: usize = 200;

#[async_trait]
impl Tool for GlobTool {
    fn definition_with_context(&self, ctx: &ToolContext) -> ToolDefinition {
        // Dynamic description based on CodeGraph availability
        let prefer_section = if ctx.codegraph_available {
            "【优先使用 code_files 的场景】
- 查看某个目录下有哪些代码文件 → code_files（更快）
- 获取项目的文件结构概览 → code_files"
        } else {
            "【glob 的适用场景】
- 查找非代码文件（配置文件、文档等）
- 搜索特定命名模式的文件"
        };

        let description = format!(
            "通过 glob 模式查找文件路径。

适用场景：
- 按文件名模式查找（如 '**/*.rs'、'src/*.toml'）
- 查找特定扩展名的所有文件
- 定位配置文件位置

{}

返回匹配路径，按修改时间排序（最新在前）。",
            prefer_section
        );

        ToolDefinition {
            name: "glob".to_string(),
            description,
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob 模式，支持 '*'、'?' 和递归 '**'"
                    },
                    "path": {
                        "type": "string",
                        "description": "搜索的基础目录（默认 '.'）"
                    }
                },
                "required": ["pattern"]
            }),
            ..Default::default()
        }
    }

    fn definition(&self) -> ToolDefinition {
        self.definition_with_context(&ToolContext::default())
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?
            .to_string();
        let path = params["path"].as_str().unwrap_or(".").to_string();

        // Show spinner while globbing - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("glob '{}' in {}", pattern, path));

        // Return result directly
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
        if let std::path::Component::Normal(s) = c
            && let Some(name) = s.to_str()
            && IGNORED.contains(&name)
        {
            return true;
        }
    }
    false
}
