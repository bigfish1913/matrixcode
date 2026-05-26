//! CLI helper functions
//!
//! Shared utilities for model resolution, skills loading, and prompt building.

use std::path::PathBuf;

use matrixcode_core::{Config, infer_provider_type, providers::ProviderType, skills::discover_skills};

/// Get default model name for anthropic provider.
pub fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
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
        roots.push(home.join(".matrix").join("skills"));
    }

    // 2. Project-local skills directories
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join(".matrix").join("skills"));
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