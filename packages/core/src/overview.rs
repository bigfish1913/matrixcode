//! Project overview generation and caching.
//!
//! The `/init` command generates a project overview file using AI analysis.
//! The overview captures the project architecture, key patterns, and development guidance.
//!
//! The overview file is stored at `MATRIX.md` in the project root.

use crate::prompt::{OverviewContext, build_overview_prompt};
use crate::providers::{ChatRequest, Message, MessageContent, Provider, Role};
use crate::truncate::find_boundary;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// Configuration Constants
// =============================================================================

/// Default filename for the cached project overview.
pub const OVERVIEW_FILENAME: &str = "MATRIX.md";
/// Directory name for matrixcode metadata.
pub const MATRIXCODE_DIR: &str = ".matrix";

// --- Token and content limits ---

/// Maximum output tokens for AI overview generation.
const MAX_OUTPUT_TOKENS: u32 = 8192;

/// Maximum characters for config file content.
const CONFIG_FILE_MAX_CHARS: usize = 2000;

/// Maximum characters for README content.
const README_MAX_CHARS: usize = 1000;

/// Maximum characters for key source file content.
const SOURCE_FILE_MAX_CHARS: usize = 3000;

/// Maximum characters for module file content.
const MODULE_FILE_MAX_CHARS: usize = 2000;

// --- Directory structure limits ---

/// Maximum depth for directory tree traversal.
const DIRECTORY_MAX_DEPTH: usize = 3;

/// Maximum items to show at root level.
const DIRECTORY_ROOT_MAX_ITEMS: usize = 15;

/// Maximum items to show at non-root levels.
const DIRECTORY_OTHER_MAX_ITEMS: usize = 10;

// --- Common file names ---

/// Default project name when root directory name cannot be determined.
const DEFAULT_PROJECT_NAME: &str = "project";

/// README filename to look for.
const README_FILENAME: &str = "README.md";

/// Source directory name for many project types.
pub const SRC_DIR: &str = "src";

/// Rust module file name.
const RUST_MOD_FILE: &str = "mod.rs";

/// Rust library entry file.
const RUST_LIB_FILE: &str = "lib.rs";

// --- Project type configuration ---

/// Configuration for a project type, including detection and key files.
pub struct ProjectTypeConfig {
    /// Human-readable type name.
    pub type_name: &'static str,
    /// Files whose presence indicates this project type (checked in order).
    pub detect_files: &'static [&'static str],
    /// Key source file paths relative to project root.
    pub key_source_files: &'static [&'static str],
}

/// All supported project type configurations.
pub const PROJECT_TYPE_CONFIGS: &[ProjectTypeConfig] = &[
    ProjectTypeConfig {
        type_name: "Rust",
        detect_files: &["Cargo.toml"],
        key_source_files: &["src/main.rs", "src/agent.rs"],
    },
    ProjectTypeConfig {
        type_name: "Go",
        detect_files: &["go.mod"],
        key_source_files: &["main.go", "cmd/main.go"],
    },
    ProjectTypeConfig {
        type_name: "Node.js/TypeScript",
        detect_files: &["package.json"],
        key_source_files: &[
            "src/index.ts",
            "src/index.js",
            "src/main.ts",
            "src/main.js",
            "src/app.ts",
            "src/app.js",
        ],
    },
    ProjectTypeConfig {
        type_name: "Python",
        detect_files: &["pyproject.toml", "requirements.txt"],
        key_source_files: &["main.py", "app.py", "__init__.py"],
    },
    ProjectTypeConfig {
        type_name: "Java (Maven)",
        detect_files: &["pom.xml"],
        key_source_files: &[],
    },
    ProjectTypeConfig {
        type_name: "Java (Gradle)",
        detect_files: &["build.gradle"],
        key_source_files: &[],
    },
    ProjectTypeConfig {
        type_name: "C/C++ (Make)",
        detect_files: &["Makefile"],
        key_source_files: &[],
    },
];

/// Unknown project type name.
const PROJECT_TYPE_UNKNOWN: &str = "Unknown";

// --- Configuration file names to scan ---

const CONFIG_FILENAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    "pom.xml",
    "build.gradle",
    "Makefile",
    "docker-compose.yml",
    "Dockerfile",
    "tsconfig.json",
    "vite.config.ts",
    "vite.config.js",
    "next.config.js",
    "nuxt.config.ts",
    "tailwind.config.js",
    "tailwind.config.ts",
    ".env.example",
];

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

        // Limit to 200 lines to prevent excessively long content
        let limited_content = content
            .lines()
            .take(200)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Some(Self { content: limited_content, path }))
    }

    /// Generate and save a new overview using AI analysis.
    /// This method collects project files and sends them to the AI for analysis.
    pub async fn generate_with_ai(project_root: &Path, provider: &dyn Provider) -> Result<Self> {
        let project_name = project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(DEFAULT_PROJECT_NAME);

        // Collect project context
        let context = collect_project_context(project_root)?;

        // Build the AI prompt
        let prompt = build_overview_prompt(&OverviewContext {
            project_name: project_name.to_string(),
            project_type: context.project_type.to_string(),
            directory_structure: context.directory_structure.clone(),
            config_files: context.config_files.clone(),
            readme: context.readme.clone(),
            source_files: context.source_files.clone(),
        });

        // Call AI API
        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt),
            }],
            tools: vec![],
            system: None,
            think: false,
            max_tokens: MAX_OUTPUT_TOKENS,
            server_tools: vec![],
            enable_caching: false, // No caching for overview generation
        };

        let response = provider
            .chat(request)
            .await
            .with_context(|| "calling AI for overview generation")?;

        // Extract content from response
        let content = extract_response_content(&response);

        // Save to file
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

/// Get the path to the overview file (directly in project root).
fn overview_path(project_root: &Path) -> PathBuf {
    project_root.join(OVERVIEW_FILENAME)
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
    "target-test",
    "build",
    "dist",
    "out",
    "bin",
    "obj",
    ".cargo",
    // IDE and editor
    ".idea",
    ".vscode",
    ".vs",
    ".claude",
    ".matrix",
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
    // Generated files
    "*.generated.*",
    "swagger.json",
    "swagger.yaml",
];

/// Check if a path component should be ignored.
pub fn should_ignore(name: &str) -> bool {
    if IGNORE_PATTERNS.contains(&name) {
        return true;
    }
    for pattern in IGNORE_PATTERNS {
        if pattern.starts_with("*.") {
            let suffix = &pattern[1..];
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

/// Project context collected for AI analysis.
struct ProjectContext {
    /// Configuration file contents (Cargo.toml, package.json, etc.)
    config_files: Vec<(String, String)>,
    /// README content (first part)
    readme: Option<String>,
    /// Directory structure summary
    directory_structure: String,
    /// Key source files content (limited)
    source_files: Vec<(String, String)>,
    /// Project type detected
    project_type: &'static str,
}

/// Collect project context for AI analysis.
fn collect_project_context(project_root: &Path) -> Result<ProjectContext> {
    // Detect project type
    let project_type = detect_project_type(project_root);

    // Collect config files
    let config_files = collect_config_files(project_root)?;

    // Get README
    let readme = read_readme(project_root)?;

    // Build directory structure
    let directory_structure = build_directory_structure(project_root)?;

    // Collect key source files
    let source_files = collect_key_source_files(project_root, project_type)?;

    Ok(ProjectContext {
        config_files,
        readme,
        directory_structure,
        source_files,
        project_type,
    })
}

/// Detect project type from configuration files.
pub fn detect_project_type(project_root: &Path) -> &'static str {
    for config in PROJECT_TYPE_CONFIGS {
        for detect_file in config.detect_files {
            if project_root.join(detect_file).exists() {
                return config.type_name;
            }
        }
    }
    PROJECT_TYPE_UNKNOWN
}

/// Collect configuration files content.
fn collect_config_files(project_root: &Path) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for filename in CONFIG_FILENAMES {
        let path = project_root.join(filename);
        if path.exists() {
            let content =
                fs::read_to_string(&path).with_context(|| format!("reading {}", filename))?;
            let truncated = truncate_content(&content, CONFIG_FILE_MAX_CHARS);
            files.push((filename.to_string(), truncated));
        }
    }

    Ok(files)
}

/// Read README.md (first part).
fn read_readme(project_root: &Path) -> Result<Option<String>> {
    let readme_path = project_root.join(README_FILENAME);
    if !readme_path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&readme_path).with_context(|| format!("reading {}", README_FILENAME))?;

    Ok(Some(truncate_content(&content, README_MAX_CHARS)))
}

/// Build directory structure string.
fn build_directory_structure(project_root: &Path) -> Result<String> {
    let mut result = String::new();
    result.push_str(&format!(
        "{}/\n",
        project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(DEFAULT_PROJECT_NAME)
    ));

    build_tree_recursive(project_root, 0, DIRECTORY_MAX_DEPTH, &mut result)?;

    Ok(result)
}

/// Build directory tree recursively.
fn build_tree_recursive(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    result: &mut String,
) -> Result<()> {
    if depth > max_depth {
        result.push_str(&format!("{}  ...\n", "  ".repeat(depth)));
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_ignore(&name) {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }

    dirs.sort();
    files.sort();

    let indent = "  ".repeat(depth);
    let max_items = if depth == 0 {
        DIRECTORY_ROOT_MAX_ITEMS
    } else {
        DIRECTORY_OTHER_MAX_ITEMS
    };

    let mut count = 0;
    for d in &dirs {
        if count >= max_items {
            result.push_str(&format!(
                "{}  ... ({} more dirs)\n",
                indent,
                dirs.len() - count
            ));
            break;
        }
        result.push_str(&format!("{}  {}/\n", indent, d));
        build_tree_recursive(&dir.join(d), depth + 1, max_depth, result)?;
        count += 1;
    }

    for f in files.iter().take(max_items - count) {
        result.push_str(&format!("{}  {}\n", indent, f));
    }

    if files.len() > max_items - count {
        result.push_str(&format!(
            "{}  ... ({} more files)\n",
            indent,
            files.len() - (max_items - count)
        ));
    }

    Ok(())
}

/// Collect key source files for analysis.
fn collect_key_source_files(
    project_root: &Path,
    project_type: &str,
) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();

    // Find the matching project type config
    let config = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == project_type);

    // Collect key source files from config
    if let Some(config) = config {
        for path_str in config.key_source_files {
            let path = project_root.join(path_str);
            if path.exists() {
                let content = fs::read_to_string(&path).ok();
                if let Some(content) = content {
                    files.push((
                        path_str.to_string(),
                        truncate_content(&content, SOURCE_FILE_MAX_CHARS),
                    ));
                }
            }
        }
    }

    // Special handling for Rust: collect lib.rs and module files
    if project_type == "Rust" {
        // Collect lib.rs
        let lib_path = project_root.join(SRC_DIR).join(RUST_LIB_FILE);
        if lib_path.exists() {
            let lib_relative = format!("{}/{}", SRC_DIR, RUST_LIB_FILE);
            let content = fs::read_to_string(&lib_path).ok();
            if let Some(content) = content {
                files.push((
                    lib_relative,
                    truncate_content(&content, SOURCE_FILE_MAX_CHARS),
                ));
            }

            // Collect module files (mod.rs in subdirectories)
            let src_path = project_root.join(SRC_DIR);
            if src_path.exists() {
                for entry in fs::read_dir(&src_path)?.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        && !should_ignore(&name)
                    {
                        let mod_path = src_path.join(&name).join(RUST_MOD_FILE);
                        if mod_path.exists() {
                            let content = fs::read_to_string(&mod_path).ok();
                            if let Some(content) = content {
                                let mod_relative =
                                    format!("{}/{}/{}", SRC_DIR, name, RUST_MOD_FILE);
                                files.push((
                                    mod_relative,
                                    truncate_content(&content, MODULE_FILE_MAX_CHARS),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(files)
}

/// Truncate content to a maximum length, respecting char boundaries.
pub fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        let end = find_boundary(content, max_len);
        let mut truncated = content[..end].to_string();
        truncated.push_str("\n... (truncated)");
        truncated
    }
}

/// Extract content from AI response.
fn extract_response_content(response: &crate::providers::ChatResponse) -> String {
    let mut content = String::new();
    for block in &response.content {
        if let crate::providers::ContentBlock::Text { text } = block {
            content.push_str(text);
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_content_respects_char_boundary() {
        // Chinese text with multibyte characters
        let text = "这是一个包含中文字符的测试文本，用于验证截断功能是否正确处理字符边界问题。";

        // Truncate at a position that would fall inside a multibyte character
        let truncated = truncate_content(text, 50);

        // Should not panic and should end with truncated marker
        assert!(truncated.contains("... (truncated)"));
        // String in Rust is always valid UTF-8, no need to check
    }

    #[test]
    fn truncate_content_preserves_short_text() {
        let short = "hello world";
        let result = truncate_content(short, 100);
        assert_eq!(result, short);
    }

    #[test]
    fn truncate_content_exact_boundary() {
        // ASCII text - every byte is a char boundary
        let text = "abcdefghijklmnopqrstuvwxyz";
        let truncated = truncate_content(text, 10);
        assert_eq!(truncated, "abcdefghij\n... (truncated)");
    }

    #[test]
    fn truncate_content_multibyte_edge() {
        // Text ending exactly at a multibyte char
        let text = "你好世界hello";
        let truncated = truncate_content(text, 12); // "你好世界" = 12 bytes
        assert!(truncated.starts_with("你好世界"));
    }
}
