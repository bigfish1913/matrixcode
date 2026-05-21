use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

/// High-performance grep tool with advanced filtering options
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: "高性能内容搜索工具，适用于任意规模代码库。支持正则表达式、文件类型过滤和多种输出模式。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "要搜索的正则表达式模式"
                    },
                    "path": {
                        "type": "string",
                        "description": "搜索的文件或目录（默认当前目录）"
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob 文件过滤模式（如 '*.ts'、'**/*.rs'）"
                    },
                    "type": {
                        "type": "string",
                        "enum": ["js", "ts", "py", "rs", "go", "java", "c", "cpp", "md", "json", "yaml", "html", "css"],
                        "description": "按文件类型搜索（映射到常用扩展名）"
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["content", "files_with_matches", "count"],
                        "default": "content",
                        "description": "输出模式：'content' 显示匹配行，'files_with_matches' 列出文件，'count' 显示匹配数"
                    },
                    "-i": {
                        "type": "boolean",
                        "default": false,
                        "description": "忽略大小写"
                    },
                    "-n": {
                        "type": "boolean",
                        "default": true,
                        "description": "显示行号"
                    },
                    "-C": {
                        "type": "integer",
                        "default": 0,
                        "description": "匹配行前后显示的上下文行数"
                    },
                    "head_limit": {
                        "type": "integer",
                        "default": 100,
                        "description": "最大返回结果数"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let pattern = params["pattern"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'pattern'"))?;
        let path = params["path"].as_str().unwrap_or(".");
        let glob_pattern = params["glob"].as_str();
        let file_type = params["type"].as_str();
        let output_mode = params["output_mode"].as_str().unwrap_or("content");
        let case_insensitive = params["-i"].as_bool().unwrap_or(false);
        let show_line_numbers = params["-n"].as_bool().unwrap_or(true);
        let context_lines = params["-C"].as_u64().unwrap_or(0) as usize;
        let head_limit = params["head_limit"].as_u64().unwrap_or(100) as usize;

        let pattern = pattern.to_string();
        let path = path.to_string();
        let glob_pattern = glob_pattern.map(|s| s.to_string());
        let file_type = file_type.map(|s| s.to_string());
        let output_mode = output_mode.to_string();

        tokio::task::spawn_blocking(move || {
            grep_search(
                &pattern, &path, glob_pattern.as_deref(),
                file_type.as_deref(), &output_mode,
                case_insensitive, show_line_numbers, context_lines, head_limit
            )
        }).await?
    }
}

/// File type to extension mapping
fn get_extensions_for_type(file_type: &str) -> Vec<&'static str> {
    match file_type {
        "js" => vec!["js", "jsx", "mjs", "cjs"],
        "ts" => vec!["ts", "tsx", "mts", "cts"],
        "py" => vec!["py", "pyw", "pyi"],
        "rs" => vec!["rs"],
        "go" => vec!["go"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "cpp" => vec!["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        "md" => vec!["md", "markdown"],
        "json" => vec!["json", "json5", "jsonc"],
        "yaml" => vec!["yaml", "yml"],
        "html" => vec!["html", "htm", "xhtml"],
        "css" => vec!["css", "scss", "sass", "less"],
        _ => vec![],
    }
}

fn grep_search(
    pattern: &str,
    path: &str,
    glob_pattern: Option<&str>,
    file_type: Option<&str>,
    output_mode: &str,
    case_insensitive: bool,
    show_line_numbers: bool,
    context_lines: usize,
    head_limit: usize,
) -> Result<String> {
    use std::fs;
    use std::path::Path;

    // Build regex with case-insensitive option
    let regex_pattern = if case_insensitive {
        regex::RegexBuilder::new(pattern).case_insensitive(true).build()?
    } else {
        regex::Regex::new(pattern)?
    };

    let root = Path::new(path);
    let mut results: Vec<String> = Vec::new();
    let mut match_count = 0;
    let mut files_with_matches: Vec<String> = Vec::new();

    // Get file extensions for type filter
    let type_extensions = file_type.map(get_extensions_for_type);

    let entries = collect_grep_files(root, glob_pattern, type_extensions.as_deref())?;

    for file_path in entries {
        if results.len() >= head_limit && output_mode == "content" {
            results.push(format!("... (limited to {} results)", head_limit));
            break;
        }

        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut file_has_match = false;
        let mut file_match_count = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            if regex_pattern.is_match(line) {
                file_has_match = true;
                file_match_count += 1;
                match_count += 1;

                if output_mode == "content" && results.len() < head_limit {
                    // Add context lines before the match first
                    if context_lines > 0 {
                        // Before context (in correct order)
                        let start_ctx = line_idx.saturating_sub(context_lines);
                        for ctx_idx in start_ctx..line_idx {
                            let ctx_line_num = ctx_idx + 1;
                            let ctx_formatted = if show_line_numbers {
                                format!("{}:{}- {}", file_path.display(), ctx_line_num, lines[ctx_idx].trim())
                            } else {
                                format!("{}- {}", file_path.display(), lines[ctx_idx].trim())
                            };
                            results.push(ctx_formatted);
                        }
                    }

                    // Then add the matching line
                    let line_num = line_idx + 1;
                    let formatted = if show_line_numbers {
                        format!("{}:{}: {}", file_path.display(), line_num, line.trim())
                    } else {
                        format!("{}: {}", file_path.display(), line.trim())
                    };
                    results.push(formatted);

                    // Add context lines after the match
                    if context_lines > 0 {
                        for ctx_idx in (line_idx + 1)..=(line_idx + context_lines).min(lines.len() - 1) {
                            let ctx_line_num = ctx_idx + 1;
                            let ctx_formatted = if show_line_numbers {
                                format!("{}:{}- {}", file_path.display(), ctx_line_num, lines[ctx_idx].trim())
                            } else {
                                format!("{}- {}", file_path.display(), lines[ctx_idx].trim())
                            };
                            results.push(ctx_formatted);
                        }
                    }
                }
            }
        }

        if file_has_match && output_mode == "files_with_matches" {
            files_with_matches.push(file_path.display().to_string());
        }

        if output_mode == "count" && file_match_count > 0 {
            results.push(format!("{}: {} matches", file_path.display(), file_match_count));
        }
    }

    // Format output based on mode
    match output_mode {
        "files_with_matches" => {
            if files_with_matches.is_empty() {
                Ok("No files matched.".to_string())
            } else {
                Ok(files_with_matches.join("\n"))
            }
        }
        "count" => {
            if results.is_empty() {
                Ok("No matches found.".to_string())
            } else {
                Ok(format!("Total: {} matches\n{}", match_count, results.join("\n")))
            }
        }
        _ => { // content
            if results.is_empty() {
                Ok("No matches found.".to_string())
            } else {
                Ok(results.join("\n"))
            }
        }
    }
}

fn collect_grep_files(
    root: &std::path::Path,
    glob_pattern: Option<&str>,
    type_extensions: Option<&[&str]>,
) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();

    if root.is_file() {
        files.push(root.to_path_buf());
        return Ok(files);
    }

    // Build glob matcher
    let glob_matcher = if let Some(pattern) = glob_pattern {
        // Handle **/*.ext patterns
        if pattern.starts_with("**/") {
            Some(glob::Pattern::new(pattern)?)
        } else {
            Some(glob::Pattern::new(pattern)?)
        }
    } else {
        None
    };

    let walker = walkdir_grep(root)?;

    for entry in walker {
        let path = entry;

        // Check glob pattern
        if let Some(ref matcher) = glob_matcher {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let relative_str = relative.to_string_lossy();
            if !matcher.matches(&relative_str) {
                // Also try just the filename
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !matcher.matches(name) {
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }

        // Check file type extensions
        if let Some(extensions) = type_extensions {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !extensions.contains(&ext) {
                continue;
            }
        }

        files.push(path);
    }

    Ok(files)
}

fn walkdir_grep(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    use std::fs;

    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    // Directories to skip
    const SKIP_DIRS: &[&str] = &[
        ".git", ".svn", ".hg",
        "node_modules", "vendor",
        "target", "build", "dist", "out",
        ".cache", ".npm", ".cargo",
        "__pycache__", ".venv", "venv",
        ".idea", ".vscode",
    ];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden files and known directories
            if name_str.starts_with('.') || SKIP_DIRS.contains(&name_str.as_ref()) {
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