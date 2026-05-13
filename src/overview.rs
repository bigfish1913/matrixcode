//! Project overview generation and caching.
//!
//! The `/init` command generates a project overview file that captures the
//! project structure and key files. On subsequent startups, this overview
//! is loaded and injected into the system prompt, avoiding the need to
//! re-scan the project each time.
//!
//! The overview file is stored at `.code-agent/OVERVIEW.md` in the project root.

use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};

/// Default filename for the cached project overview.
pub const OVERVIEW_FILENAME: &str = "matrix.md";
/// Directory name for code-agent metadata.
pub const CODE_AGENT_DIR: &str = ".code-agent";

/// Project overview containing the generated summary.
#[derive(Debug, Clone)]
pub struct ProjectOverview {
    /// The rendered markdown content.
    pub content: String,
    /// Path to the overview file (for cache invalidation info).
    pub path: PathBuf,
}

impl ProjectOverview {
    /// Load the overview from the project root if it exists.
    /// Returns `None` if the file doesn't exist.
    pub fn load(project_root: &Path) -> Result<Option<Self>> {
        let path = overview_path(project_root);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading overview file {}", path.display()))?;
        Ok(Some(Self { content, path }))
    }

    /// Generate and save a new overview for the project.
    /// Returns the generated overview content.
    pub fn generate(project_root: &Path) -> Result<Self> {
        let content = generate_overview_content(project_root)?;
        let dir = project_root.join(CODE_AGENT_DIR);
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating directory {}", dir.display()))?;
        let path = overview_path(project_root);
        fs::write(&path, &content)
            .with_context(|| format!("writing overview file {}", path.display()))?;
        Ok(Self { content, path })
    }

    /// Delete the overview file if it exists.
    pub fn clear(project_root: &Path) -> Result<()> {
        let path = overview_path(project_root);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("removing overview file {}", path.display()))?;
        }
        Ok(())
    }

    /// Check if an overview exists for the project.
    pub fn exists(project_root: &Path) -> bool {
        overview_path(project_root).exists()
    }

    /// Get the path to the overview file.
    pub fn path(project_root: &Path) -> PathBuf {
        overview_path(project_root)
    }
}

/// Get the path to the overview file.
fn overview_path(project_root: &Path) -> PathBuf {
    project_root.join(CODE_AGENT_DIR).join(OVERVIEW_FILENAME)
}

/// Patterns to ignore when scanning the project.
const IGNORE_PATTERNS: &[&str] = &[
    // Version control
    ".git",
    ".svn",
    ".hg",
    // Dependencies
    "node_modules",
    "vendor",
    // Build outputs
    "target",
    "build",
    "dist",
    "out",
    "bin",
    "obj",
    // IDE and editor
    ".idea",
    ".vscode",
    ".vs",
    // Cache and temp
    ".cache",
    "__pycache__",
    "*.pyc",
    ".DS_Store",
    "Thumbs.db",
    // Lock files (usually large and not informative)
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    // Code-agent metadata
    CODE_AGENT_DIR,
];

/// Check if a path component should be ignored.
fn should_ignore(name: &str) -> bool {
    // Check exact matches
    if IGNORE_PATTERNS.contains(&name) {
        return true;
    }
    // Check glob patterns (simple suffix match for now)
    for pattern in IGNORE_PATTERNS {
        if pattern.starts_with("*.") {
            let suffix = &pattern[1..]; // "*" is 1 char
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

/// Generate the overview content by scanning the project.
fn generate_overview_content(project_root: &Path) -> Result<String> {
    let mut content = String::new();
    
    // Header
    content.push_str("# 项目概览\n\n");
    content.push_str(&format!("> 生成时间: {}\n\n", chrono_timestamp()));
    content.push_str("此文件由 `/init` 命令自动生成，包含项目结构和关键文件概览。\n\n");
    
    // Project root name
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    content.push_str(&format!("## 项目: {}\n\n", project_name));
    
    // Directory structure
    content.push_str("## 目录结构\n\n");
    content.push_str("```\n");
    let tree = build_tree(project_root, 0, 4)?; // max depth of 4
    content.push_str(&tree);
    content.push_str("```\n\n");
    
    // Key files detection
    content.push_str("## 关键文件\n\n");
    let key_files = detect_key_files(project_root)?;
    for (name, desc) in &key_files {
        content.push_str(&format!("- **{}**: {}\n", name, desc));
    }
    
    if key_files.is_empty() {
        content.push_str("_未检测到常见配置文件_\n");
    }
    
    content.push('\n');
    
    // Language detection
    content.push_str("## 技术栈\n\n");
    let languages = detect_languages(project_root)?;
    if languages.is_empty() {
        content.push_str("_未能检测_\n");
    } else {
        for (lang, count) in &languages {
            content.push_str(&format!("- {}: {} 个文件\n", lang, count));
        }
    }
    
    Ok(content)
}

/// Build a tree representation of the directory structure.
fn build_tree(dir: &Path, depth: usize, max_depth: usize) -> Result<String> {
    if depth > max_depth {
        return Ok("...\n".to_string());
    }
    
    let mut result = String::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(result),
    };
    
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        
        if should_ignore(&name_str) {
            continue;
        }
        
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        
        if file_type.is_dir() {
            dirs.push(name_str.into_owned());
        } else if file_type.is_file() {
            files.push(name_str.into_owned());
        }
    }
    
    dirs.sort();
    files.sort();
    
    let indent = "  ".repeat(depth);
    
    // Limit output size
    let max_entries = 20;
    let mut count = 0;
    
    for d in &dirs {
        if count >= max_entries {
            result.push_str(&format!("{}... (更多目录)\n", indent));
            break;
        }
        result.push_str(&format!("{}{}/\n", indent, d));
        if depth + 1 <= max_depth {
            let subtree = build_tree(&dir.join(d), depth + 1, max_depth)?;
            result.push_str(&subtree);
        }
        count += 1;
    }
    
    for f in &files {
        if count >= max_entries {
            result.push_str(&format!("{}... (更多文件)\n", indent));
            break;
        }
        result.push_str(&format!("{}{}\n", indent, f));
        count += 1;
    }
    
    Ok(result)
}

/// Detect key configuration files in the project.
fn detect_key_files(project_root: &Path) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();
    
    let key_files = [
        ("Cargo.toml", "Rust 项目配置"),
        ("package.json", "Node.js 项目配置"),
        ("pyproject.toml", "Python 项目配置"),
        ("requirements.txt", "Python 依赖"),
        ("go.mod", "Go 模块配置"),
        ("pom.xml", "Maven 项目配置"),
        ("build.gradle", "Gradle 项目配置"),
        ("Makefile", "构建脚本"),
        ("Dockerfile", "Docker 构建文件"),
        ("docker-compose.yml", "Docker Compose 配置"),
        ("README.md", "项目说明"),
        ("LICENSE", "许可证"),
        (".env.example", "环境变量示例"),
    ];
    
    for (filename, description) in &key_files {
        if project_root.join(filename).exists() {
            result.push((filename.to_string(), description.to_string()));
        }
    }
    
    Ok(result)
}

/// Detect programming languages by file extension count.
fn detect_languages(project_root: &Path) -> Result<Vec<(String, usize)>> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    
    fn count_files(dir: &Path, counts: &mut std::collections::HashMap<String, usize>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            if should_ignore(&name_str) {
                continue;
            }
            
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            
            if file_type.is_dir() {
                count_files(&entry.path(), counts);
            } else if file_type.is_file() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    let lang = match ext {
                        "rs" => "Rust",
                        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
                        "ts" | "tsx" => "TypeScript",
                        "py" => "Python",
                        "go" => "Go",
                        "java" => "Java",
                        "kt" | "kts" => "Kotlin",
                        "rb" => "Ruby",
                        "php" => "PHP",
                        "c" | "h" => "C",
                        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "C++",
                        "cs" => "C#",
                        "swift" => "Swift",
                        "scala" => "Scala",
                        "sh" | "bash" => "Shell",
                        "sql" => "SQL",
                        "html" | "htm" => "HTML",
                        "css" | "scss" | "sass" | "less" => "CSS",
                        "json" | "yaml" | "yml" | "toml" | "xml" => "配置",
                        "md" => "Markdown",
                        _ => continue,
                    };
                    *counts.entry(lang.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    
    count_files(project_root, &mut counts);
    
    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Limit output
    result.truncate(10);
    
    Ok(result)
}

/// Generate a timestamp for the overview file.
fn chrono_timestamp() -> String {
    // Use simple ISO 8601 format without requiring chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple conversion to datetime (approximate)
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    // Unix epoch is 1970-01-01
    let year = 1970 + (days / 365);
    let month = ((days % 365) / 30) + 1;
    let day = (days % 30) + 1;
    format!("{}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn should_ignore_patterns() {
        assert!(should_ignore(".git"));
        assert!(should_ignore("node_modules"));
        assert!(should_ignore("target"));
        assert!(should_ignore("__pycache__"));
        assert!(should_ignore("Cargo.lock"));
        assert!(!should_ignore("src"));
        assert!(!should_ignore("main.rs"));
    }

    #[test]
    fn overview_load_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let result = ProjectOverview::load(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn overview_generate_creates_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        
        let overview = ProjectOverview::generate(tmp.path()).unwrap();
        assert!(overview.content.contains("项目概览"));
        assert!(overview.path.exists());
    }

    #[test]
    fn overview_load_returns_content() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        
        ProjectOverview::generate(tmp.path()).unwrap();
        let loaded = ProjectOverview::load(tmp.path()).unwrap().unwrap();
        assert!(loaded.content.contains("项目概览"));
    }

    #[test]
    fn overview_clear_removes_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        
        ProjectOverview::generate(tmp.path()).unwrap();
        assert!(ProjectOverview::exists(tmp.path()));
        
        ProjectOverview::clear(tmp.path()).unwrap();
        assert!(!ProjectOverview::exists(tmp.path()));
    }
}