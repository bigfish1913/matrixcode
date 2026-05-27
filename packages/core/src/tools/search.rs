use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::time::{Duration, timeout};

use super::{Tool, ToolDefinition};

pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search".to_string(),
            description: r#"在文件内容中搜索匹配的文本模式。

适用场景：
- 搜索文本内容（错误消息、日志、注释）
- 搜索字符串常量
- 不确定目标是否是代码符号

【优先使用 code_search 的场景】
如果 CodeGraph 可用（系统提示中有 CodeGraph 工具），以下场景应该用 code_search：
- 查找函数/类/方法/变量的定义 → code_search（快 10-100 倍）
- 查找符号的调用关系 → code_callers/callees
- 查找谁调用了某函数 → code_callers

【使用 search 的场景】
- 搜索非代码文本（错误消息、日志文本）
- CodeGraph 未初始化或不可用
- 需要搜索特定文件类型的内容"#.to_string(),
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
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?;
        let path = params["path"].as_str().unwrap_or(".");
        let glob_pattern = params["glob"].as_str();

        let pattern = pattern.to_string();
        let path = path.to_string();
        let glob_pattern = glob_pattern.map(|s| s.to_string());

        // Use timeout to prevent hanging on large directories
        timeout(Duration::from_secs(30), async {
            tokio::task::spawn_blocking(move || {
                search_files(&pattern, &path, glob_pattern.as_deref())
            })
            .await?
        })
        .await
        .map_err(|_| anyhow::anyhow!("Search timeout (30s) - directory may be too large"))?
    }
}

/// Maximum files to search before stopping.
const MAX_FILES: usize = 500;

fn search_files(pattern: &str, path: &str, glob_pattern: Option<&str>) -> Result<String> {
    use std::fs;
    use std::path::Path;

    let regex = regex::Regex::new(pattern)?;
    let mut results = Vec::new();
    let root = Path::new(path);

    let entries = collect_files(root, glob_pattern)?;

    for file_path in entries {
        // Skip very large files (> 1MB)
        match fs::metadata(&file_path) {
            Ok(meta) if meta.len() > 1_000_000 => continue,
            Err(_) => continue,
            Ok(_) => {}
        }

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

    let glob_matcher = glob_pattern.map(glob::Pattern::new).transpose()?;

    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden dirs and common large directories
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "dist"
                || name_str == "build"
                || name_str == ".git"
            {
                continue;
            }

            // Check glob pattern for files
            if let Some(ref matcher) = glob_matcher
                && path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !matcher.matches(name)
            {
                continue;
            }

            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                files.push(path);
                // Limit number of files to search
                if files.len() >= MAX_FILES {
                    return Ok(files);
                }
            }
        }
    }

    Ok(files)
}
