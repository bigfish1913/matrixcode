//! CLI helper functions
//!
//! Shared utilities for model resolution, skills loading, MCP integration, and prompt building.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use matrixcode_core::{
    Config, infer_provider_type, providers::ProviderType, skills::discover_skills,
    constants::MATRIX_DIR,
};

use crate::constants::DEFAULT_MODEL;

/// Get default model name for anthropic provider.
pub fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Get default base URL for anthropic provider.
pub fn default_base_url() -> String {
    "https://api.anthropic.com".to_string()
}

/// Resolve provider type from config, env, or model name.
pub fn resolve_provider(config: &Config, model: &str) -> ProviderType {
    let provider_str = config
        .provider
        .as_ref()
        .cloned()
        .or_else(|| std::env::var("PROVIDER").ok());

    provider_str
        .map(|p| match p.to_lowercase().as_str() {
            "openai" => ProviderType::OpenAI,
            _ => ProviderType::Anthropic,
        })
        .unwrap_or_else(|| infer_provider_type(model))
}

/// Resolve model from config, env, or default.
pub fn resolve_model(config: &Config) -> String {
    config
        .model
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(default_model)
}

/// Resolve base URL from config, env, or default.
pub fn resolve_base_url(config: &Config) -> String {
    config
        .base_url
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(default_base_url)
}

/// Resolve model with optional override, then config, env, or default.
pub fn resolve_model_with_override(override_model: Option<String>, config: &Config) -> String {
    override_model
        .or(config.model.clone())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(default_model)
}

/// Get model name with source annotation for status display.
pub fn model_with_source(config: &Config) -> String {
    if let Some(model) = &config.model {
        format!("{} (config)", model)
    } else if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
        format!("{} (env)", model)
    } else {
        format!("{} (default)", default_model())
    }
}

/// Load skills from directories (MatrixCode only)
pub fn load_skills(extra_dirs: &[PathBuf]) -> Vec<matrixcode_core::skills::Skill> {
    // Build list of skill directories to search (in priority order)
    let mut roots: Vec<PathBuf> = Vec::new();

    // 1. User's global skills directory (~/.matrix/skills)
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(MATRIX_DIR).join("skills"));
    }

    // 2. Project-local skills directories
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join(MATRIX_DIR).join("skills"));
        roots.push(cwd.join("skills"));
    }

    // 3. Extra directories from CLI option
    roots.extend(extra_dirs.iter().cloned());

    discover_skills(&roots)
}

/// Parse MCP server spec from CLI --mcp parameter.
/// Format: name:command args (e.g., playwright:npx -y @playwright/mcp@latest)
/// Or: command args (e.g., npx -y @modelcontextprotocol/server-filesystem)
fn parse_mcp_spec(spec: &str) -> Option<(String, String, Vec<String>)> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    
    // Try name:command args format
    if let Some((name, rest)) = spec.split_once(':') {
        let name = name.trim().to_string();
        let rest = rest.trim();
        
        // Split by whitespace (works for simple cases)
        let parts: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();
        
        if parts.is_empty() {
            return None;
        }
        
        let command = parts[0].clone();
        let args = parts[1..].to_vec();
        
        return Some((name, command, args));
    }
    
    // Try command args format (use command as name)
    let parts: Vec<String> = spec.split_whitespace().map(|s| s.to_string()).collect();
    
    if parts.is_empty() {
        return None;
    }
    
    let command = parts[0].clone();
    let name = command.clone();
    let args = parts[1..].to_vec();
    
    Some((name, command, args))
}

/// Load MCP tools from CLI params and config files.
///
/// Priority order:
/// 1. CLI --mcp params (highest)
/// 2. Project .matrix/mcp.toml or mcp.json
/// 3. Global ~/.matrix/mcp.toml or mcp.json
///
/// Returns async function to be called in tokio runtime.
pub fn prepare_mcp_tools(
    cli_mcp_specs: &[String],
    no_mcp: bool,
    project_path: Option<&PathBuf>,
) -> Vec<(String, matrixcode_core::mcp::McpServerConfig)> {
    if no_mcp {
        return Vec::new();
    }

    let mut servers: Vec<(String, matrixcode_core::mcp::McpServerConfig)> = Vec::new();

    // 1. Parse CLI --mcp params
    for spec in cli_mcp_specs {
        if let Some((name, command, args)) = parse_mcp_spec(spec) {
            servers.push((
                name.clone(),
                matrixcode_core::mcp::McpServerConfig::stdio(command, args)
                    .with_name(name),
            ));
        }
    }

    // 2. Load from config files (project + global)
    if servers.is_empty() {
        // Only load config files if no CLI params provided
        let config = if let Some(path) = project_path {
            matrixcode_core::mcp::load_mcp_config(path)
        } else {
            matrixcode_core::mcp::load_mcp_config(&std::env::current_dir().unwrap_or_default())
        };

        for (key, server_config) in config.enabled_servers() {
            servers.push((key, server_config.clone()));
        }
    }

    servers
}

/// Auto-detect project languages from project files.
///
/// Checks common project files to determine which languages are used.
/// Also checks immediate subdirectories for monorepo/multi-project scenarios.
///
/// Returns set of language identifiers (e.g., "rust", "go", "typescript").
fn detect_project_languages(project_path: &Path) -> HashSet<&'static str> {
    let mut languages = HashSet::new();

    // First check root directory
    check_languages_in_dir(project_path, &mut languages);

    // If no languages found at root, check immediate subdirectories
    // This handles monorepo scenarios like:
    // - matrixdocs/ (root) -> compile-server/go.mod (subdir)
    // - matrixcode/ (root) -> packages/cli/Cargo.toml (subdir)
    if languages.is_empty() {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                let subdir = entry.path();
                if subdir.is_dir() {
                    // Skip hidden directories and common non-project directories
                    let dir_name = subdir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "vendor" {
                        continue;
                    }
                    check_languages_in_dir(&subdir, &mut languages);
                }
            }
        }
    }

    languages
}

/// Check for language marker files in a single directory.
fn check_languages_in_dir(dir: &Path, languages: &mut HashSet<&'static str>) {
    // Rust: Cargo.toml
    if dir.join("Cargo.toml").exists() {
        languages.insert("rust");
    }

    // Go: go.mod
    if dir.join("go.mod").exists() {
        languages.insert("go");
    }

    // TypeScript: package.json + tsconfig.json
    if dir.join("package.json").exists() {
        if dir.join("tsconfig.json").exists() {
            languages.insert("typescript");
        } else {
            languages.insert("javascript");
        }
    }

    // Python: pyproject.toml, setup.py, requirements.txt
    if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("requirements.txt").exists() {
        languages.insert("python");
    }

    // C/C++: CMakeLists.txt, Makefile, or .c/.cpp files
    if dir.join("CMakeLists.txt").exists()
        || dir.join("Makefile").exists()
        || has_cpp_files(dir) {
        languages.insert("cpp");
    }

    // Java: pom.xml, build.gradle
    if dir.join("pom.xml").exists()
        || dir.join("build.gradle").exists() {
        languages.insert("java");
    }
}

/// Check if project has C/C++ source files.
fn has_cpp_files(project_path: &Path) -> bool {
    // Check common source directories
    let dirs = ["src", "lib", "include", "."];
    for dir in dirs {
        let dir_path = project_path.join(dir);
        if dir_path.exists() {
            // Check for .c, .cpp, .cc, .cxx files
            if let Ok(entries) = std::fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    if name.ends_with(".c") 
                        || name.ends_with(".cpp")
                        || name.ends_with(".cc")
                        || name.ends_with(".cxx") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Auto-detect available LSP servers for the project.
///
/// Checks common LSP servers and filters by project languages.
/// No configuration needed - just detect and use.
///
/// Returns list of (name, config) pairs for available servers.
pub fn prepare_lsp_servers(
    config: &Config,
    project_path: Option<&Path>,
    start_path: Option<&Path>,
) -> Vec<(String, matrixcode_core::lsp::LspServerConfig)> {
    use matrixcode_core::lsp::LspServerConfig;

    // Check if LSP is enabled globally
    if !config.enable_lsp {
        log::info!("LSP disabled by config (enable_lsp=false)");
        println!("[lsp: disabled by config (enable_lsp=false)]");
        return vec![];
    }
    println!("[lsp: enabled]");

    // Detect project languages from both project_root and start_path
    // This handles the case where a Go project is in a subdirectory of a Rust monorepo
    let mut project_languages = HashSet::new();

    if let Some(p) = project_path {
        project_languages.extend(detect_project_languages(p));
    }

    // Also detect from start_path (original directory where user ran the command)
    // This is crucial for detecting the correct language in monorepo scenarios
    if let Some(s) = start_path {
        let start_languages = detect_project_languages(s);
        // If start_path detected different languages, prefer those (user is working there)
        if !start_languages.is_empty() {
            log::info!("Languages detected from start_path: {:?}", start_languages);
            project_languages.extend(start_languages);
        }
    }

    // If no languages detected at all, don't load any LSP (show gray status)
    if project_languages.is_empty() {
        log::info!("No project languages detected, skipping LSP server loading");
        return vec![];
    }

    // Common LSP servers to check
    let common_servers = [
        // Rust
        ("rust-analyzer", "rust", "rust-analyzer", vec![]),
        // TypeScript/JavaScript
        ("typescript-language-server", "typescript", "typescript-language-server", vec!["--stdio".to_string()]),
        // Python
        ("pylsp", "python", "pylsp", vec![]),
        ("pyright", "python", "pyright", vec!["--stdio".to_string()]),
        // Go
        ("gopls", "go", "gopls", vec![]),
        // C/C++
        ("clangd", "cpp", "clangd", vec![]),
        // Java
        ("jdtls", "java", "jdtls", vec![]),
    ];

    let mut servers: Vec<(String, LspServerConfig)> = Vec::new();

    for (name, language, command, args) in &common_servers {
        // Filter: must be installed AND in project languages
        let in_project = project_languages.contains(language);

        if in_project && is_command_available(command) {
            log::info!("LSP server '{}' detected and available for project", name);
            servers.push((
                name.to_string(),
                LspServerConfig::new(command.to_string(), language.to_string())
                    .with_args(args.clone()),
            ));
        }
    }

    servers
}

/// Check if a command is available in the system.
fn is_command_available(command: &str) -> bool {
    // Try to find the command using 'which' on Unix or 'where' on Windows
    if cfg!(target_os = "windows") {
        // On Windows, use cmd /C where command
        std::process::Command::new("cmd")
            .args(["/C", "where", command])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        // On Unix, use which command
        std::process::Command::new("sh")
            .args(["-c", &format!("which {}", command)])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// Build quick action prompt from action type and file
pub fn build_quick_action_prompt(action: &str, file: Option<&String>) -> String {
    match action {
        "explain" => {
            if let Some(f) = file {
                format!(
                    "Please explain the code in {} in detail, including its purpose, structure, and key concepts.",
                    f
                )
            } else {
                "Please explain the code in detail.".to_string()
            }
        }
        "fix" => {
            if let Some(f) = file {
                format!("Please analyze {} for bugs or issues and fix them.", f)
            } else {
                "Please analyze the code for bugs or issues and fix them.".to_string()
            }
        }
        "refactor" => {
            if let Some(f) = file {
                format!(
                    "Please refactor {} to improve its structure, readability, and maintainability.",
                    f
                )
            } else {
                "Please refactor the code to improve its structure.".to_string()
            }
        }
        "test" => {
            if let Some(f) = file {
                format!("Please write unit tests for the code in {}.", f)
            } else {
                "Please write unit tests for the code.".to_string()
            }
        }
        "doc" | "document" => {
            if let Some(f) = file {
                format!("Please add documentation and comments to {}.", f)
            } else {
                "Please add documentation and comments to the code.".to_string()
            }
        }
        "optimize" => {
            if let Some(f) = file {
                format!("Please optimize {} for better performance.", f)
            } else {
                "Please optimize the code for better performance.".to_string()
            }
        }
        "review" => {
            if let Some(f) = file {
                format!(
                    "Please review {} and provide feedback on code quality, potential issues, and improvements.",
                    f
                )
            } else {
                "Please review the code and provide feedback.".to_string()
            }
        }
        other => {
            if let Some(f) = file {
                format!("{}: {}", other, f)
            } else {
                other.to_string()
            }
        }
    }
}