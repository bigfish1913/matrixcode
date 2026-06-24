//! CLI helper functions
//!
//! Shared utilities for model resolution, skills loading, and prompt building.

use std::path::{Path, PathBuf};

use matrixcode_core::{
    Config, infer_provider_type, providers::ProviderType, skills::discover_skills,
    constants::MATRIX_DIR, mcp::McpServerConfig, lsp::{LspServerConfig, load_lsp_config},
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

/// Prepare MCP servers from CLI params and config files
///
/// Priority:
/// 1. CLI --mcp params (highest)
/// 2. Project .matrix/mcp.toml or mcp.json
/// 3. Global ~/.matrix/mcp.toml or mcp.json
pub fn prepare_mcp_tools(
    cli_mcp_specs: &[String],
    no_mcp: bool,
    project_path: Option<&PathBuf>,
) -> Vec<(String, McpServerConfig)> {
    if no_mcp {
        return Vec::new();
    }

    let mut servers = Vec::new();

    // 1. CLI params (highest priority)
    for spec in cli_mcp_specs {
        if let Some((name, config)) = parse_mcp_spec(spec) {
            servers.push((name, config));
        }
    }

    // 2. Project config
    if let Some(path) = project_path {
        let project_config = path.join(MATRIX_DIR).join("mcp.json");
        if project_config.exists() {
            if let Ok(content) = std::fs::read_to_string(&project_config) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(arr) = config.get("servers").and_then(|v| v.as_array()) {
                        for server in arr {
                            if let Some(obj) = server.as_object() {
                                let name = obj.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let command = obj.get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let args = obj.get("args")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect())
                                    .unwrap_or_default();
                                servers.push((name, McpServerConfig::stdio(command, args)));
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Global config
    if let Some(home) = dirs::home_dir() {
        let global_config = home.join(MATRIX_DIR).join("mcp.json");
        if global_config.exists() {
            if let Ok(content) = std::fs::read_to_string(&global_config) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(arr) = config.get("servers").and_then(|v| v.as_array()) {
                        for server in arr {
                            if let Some(obj) = server.as_object() {
                                let name = obj.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let command = obj.get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let args = obj.get("args")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect())
                                    .unwrap_or_default();
                                servers.push((name, McpServerConfig::stdio(command, args)));
                            }
                        }
                    }
                }
            }
        }
    }

    servers
}

/// Parse MCP server spec from CLI --mcp parameter
fn parse_mcp_spec(spec: &str) -> Option<(String, McpServerConfig)> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }

    // Try name:command args format
    if let Some((name, rest)) = spec.split_once(':') {
        let name = name.trim().to_string();
        let rest = rest.trim();
        let parts: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();

        if parts.is_empty() {
            return None;
        }

        let command = parts[0].clone();
        let args = parts[1..].to_vec();
        return Some((name, McpServerConfig::stdio(command, args)));
    }

    // Try command args format (auto-generate name from command)
    let parts: Vec<String> = spec.split_whitespace().map(|s| s.to_string()).collect();
    if parts.is_empty() {
        return None;
    }

    let command = parts[0].clone();
    let args = parts[1..].to_vec();
    let name = command.split('/').last()
        .unwrap_or(&command)
        .split('@').next()
        .unwrap_or(&command)
        .to_string();

    Some((name, McpServerConfig::stdio(command, args)))
}

/// Prepare LSP servers from config files
///
/// Checks .matrix/lsp.json in project and home directories
pub fn prepare_lsp_servers(
    _config: &Config,
    project_path: Option<&Path>,
    start_path: Option<&Path>,
) -> Vec<(String, LspServerConfig)> {
    let mut servers = Vec::new();

    // Try to load from project config first
    if let Some(path) = project_path {
        let lsp_config = load_lsp_config(path);
        for server_config in lsp_config.enabled_servers() {
            servers.push((server_config.command.clone(), server_config.clone()));
        }
    }

    // Try start_path if project_path didn't find anything
    if servers.is_empty() && start_path.is_some() && start_path != project_path {
        if let Some(path) = start_path {
            let lsp_config = load_lsp_config(path);
            for server_config in lsp_config.enabled_servers() {
                servers.push((server_config.command.clone(), server_config.clone()));
            }
        }
    }

    // Try global config if nothing found
    if servers.is_empty() {
        if let Some(home) = dirs::home_dir() {
            let global_config = home.join(MATRIX_DIR).join("lsp.json");
            if global_config.exists() {
                let lsp_config = load_lsp_config(&global_config);
                for server_config in lsp_config.enabled_servers() {
                    servers.push((server_config.command.clone(), server_config.clone()));
                }
            }
        }
    }

    servers
}