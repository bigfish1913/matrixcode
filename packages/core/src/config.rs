//! Configuration loading for matrixcode.
//!
//! Universal naming style (no provider-specific prefixes):
//! - api_key
//! - base_url
//! - model
//! - plan_model
//! - compress_model
//!
//! Also supports Claude Code style aliases for compatibility:
//! - ANTHROPIC_AUTH_TOKEN (alias for api_key)
//! - ANTHROPIC_BASE_URL (alias for base_url)
//! - ANTHROPIC_MODEL (alias for model)
//!
//! Priority:
//! 1. CLI arguments (highest priority)
//! 2. ~/.matrix/config.json (matrixcode's own config)
//! 3. Environment variables
//! 4. Defaults

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Matrixcode configuration file structure.
/// Uses universal naming (no ANTHROPIC_ prefix).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixConfig {
    /// LLM provider: "anthropic" or "openai"
    #[serde(default)]
    pub provider: Option<String>,

    /// API key (universal naming, also supports ANTHROPIC_AUTH_TOKEN alias)
    #[serde(default, alias = "ANTHROPIC_AUTH_TOKEN")]
    pub api_key: Option<String>,

    /// Base URL for API endpoint
    #[serde(default, alias = "ANTHROPIC_BASE_URL")]
    pub base_url: Option<String>,

    /// Main model name
    #[serde(default, alias = "ANTHROPIC_MODEL")]
    pub model: Option<String>,

    /// Enable extended thinking
    #[serde(default = "default_true")]
    pub think: bool,

    /// Enable markdown rendering
    #[serde(default = "default_true")]
    pub markdown: bool,

    /// Maximum output tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Context size
    #[serde(default)]
    pub context_size: Option<u32>,

    /// Multi-model configuration
    #[serde(default)]
    pub multi_model: Option<bool>,

    /// Plan/reasoning model
    #[serde(default, alias = "ANTHROPIC_REASONING_MODEL")]
    pub plan_model: Option<String>,

    /// Compress/haiku model
    #[serde(default, alias = "ANTHROPIC_DEFAULT_HAIKU_MODEL")]
    pub compress_model: Option<String>,

    /// Fast model
    #[serde(default)]
    pub fast_model: Option<String>,

    /// Approve mode: "ask", "auto", "strict"
    #[serde(default = "default_approve_mode")]
    pub approve_mode: Option<String>,

    /// Extra HTTP headers to add to API requests
    /// Format: {"Header-Name": "header-value"}
    #[serde(default)]
    pub extra_headers: Option<HashMap<String, String>>,
}

fn default_true() -> bool {
    true
}
fn default_max_tokens() -> u32 {
    16384
}
fn default_approve_mode() -> Option<String> {
    Some("ask".to_string())
}

/// Type alias for compatibility
pub type Config = MatrixConfig;

/// Claude Code settings.json structure (for fallback).
#[derive(Debug, Clone, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    env: Option<ClaudeEnv>,
}

/// Environment variables from Claude Code settings.
#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
struct ClaudeEnv {
    #[serde(default)]
    ANTHROPIC_AUTH_TOKEN: Option<String>,
    #[serde(default)]
    ANTHROPIC_BASE_URL: Option<String>,
    #[serde(default)]
    ANTHROPIC_MODEL: Option<String>,
    #[serde(default)]
    ANTHROPIC_DEFAULT_HAIKU_MODEL: Option<String>,
    #[serde(default)]
    ANTHROPIC_REASONING_MODEL: Option<String>,
}

impl MatrixConfig {
    /// Get the home directory.
    fn home_dir() -> Option<PathBuf> {
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    /// Path to matrixcode config file.
    pub fn matrix_config_path() -> Option<PathBuf> {
        Self::home_dir().map(|h| h.join(".matrix").join("config.json"))
    }

    /// Path to Claude Code settings file.
    pub fn claude_settings_path() -> Option<PathBuf> {
        Self::home_dir().map(|h| h.join(".claude").join("settings.json"))
    }

    /// Load matrixcode's own config file.
    fn load_matrix_config() -> Option<Self> {
        let path = Self::matrix_config_path()?;
        if !path.exists() {
            return None;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read ~/.matrix/config.json: {}", e);
                return None;
            }
        };
        let config: Self = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to parse ~/.matrix/config.json: {}", e);
                return None;
            }
        };

        Some(config)
    }

    /// Load Claude Code settings as fallback.
    fn load_claude_settings() -> Option<Self> {
        let path = Self::claude_settings_path()?;
        if !path.exists() {
            return None;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read ~/.claude/settings.json: {}", e);
                return None;
            }
        };
        let settings: ClaudeSettings = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to parse ~/.claude/settings.json: {}", e);
                return None;
            }
        };

        let env = settings.env?;
        Some(Self {
            provider: Some("anthropic".to_string()),
            api_key: env.ANTHROPIC_AUTH_TOKEN,
            base_url: env.ANTHROPIC_BASE_URL,
            model: env.ANTHROPIC_MODEL,
            think: true,
            markdown: true,
            max_tokens: 16384,
            context_size: None,
            multi_model: None,
            plan_model: env.ANTHROPIC_REASONING_MODEL,
            compress_model: env.ANTHROPIC_DEFAULT_HAIKU_MODEL,
            fast_model: None,
            approve_mode: Some("ask".to_string()),
            extra_headers: None,
        })
    }

    /// Load configuration with fallback chain.
    /// Priority: ~/.matrix/config.json > ~/.claude/settings.json > env vars > defaults
    pub fn load() -> Self {
        let matrix_config = Self::load_matrix_config();
        let claude_config = Self::load_claude_settings();

        match (matrix_config, claude_config) {
            (Some(mx), Some(cc)) => {
                // Merge: matrix config takes precedence, fill missing from Claude
                let needs_fallback = mx.api_key.is_none() || mx.model.is_none() || mx.base_url.is_none();
                if needs_fallback {
                    println!("[config: ~/.matrix/config.json + fallback from ~/.claude/settings.json]");
                } else {
                    println!("[config: ~/.matrix/config.json]");
                }
                Self {
                    provider: mx.provider.or(cc.provider),
                    api_key: mx.api_key.or(cc.api_key),
                    base_url: mx.base_url.or(cc.base_url),
                    model: mx.model.or(cc.model),
                    think: mx.think,
                    markdown: mx.markdown,
                    max_tokens: mx.max_tokens,
                    context_size: mx.context_size.or(cc.context_size),
                    multi_model: mx.multi_model.or(cc.multi_model),
                    plan_model: mx.plan_model.or(cc.plan_model),
                    compress_model: mx.compress_model.or(cc.compress_model),
                    fast_model: mx.fast_model.or(cc.fast_model),
                    approve_mode: mx.approve_mode.or(Some("ask".to_string())),
                    extra_headers: mx.extra_headers.or(cc.extra_headers),
                }
            }
            (Some(mx), None) => {
                println!("[config: ~/.matrix/config.json]");
                Self {
                    approve_mode: mx.approve_mode.or(Some("ask".to_string())),
                    ..mx
                }
            }
            (None, Some(cc)) => {
                println!("[config: ~/.claude/settings.json (Claude Code fallback)]");
                cc
            }
            (None, None) => {
                println!("[config: using environment variables and defaults]");
                Self::default()
            }
        }
    }

    /// Get API key, with fallback to environment variable.
    /// Universal env var: API_KEY (also supports ANTHROPIC_AUTH_TOKEN for compatibility)
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        // Try universal env var first
        let env_key = env::var("API_KEY").ok()
            // Then provider-specific env vars
            .or_else(|| match provider {
                "openai" => env::var("OPENAI_API_KEY").ok(),
                _ => env::var("ANTHROPIC_AUTH_TOKEN").ok()
                    .or_else(|| env::var("ANTHROPIC_API_KEY").ok()),
            });
        // Finally config file
        env_key.or(self.api_key.clone())
    }

    /// Get model name, with fallback to environment variable.
    /// Universal env var: MODEL (also supports ANTHROPIC_MODEL for compatibility)
    pub fn get_model(&self, provider: &str) -> String {
        env::var("MODEL").ok()
            .or_else(|| env::var("ANTHROPIC_MODEL").ok())
            .or_else(|| env::var("MODEL_NAME").ok())
            .or(self.model.clone())
            .unwrap_or_else(|| match provider {
                "openai" => "gpt-4o".to_string(),
                _ => "claude-sonnet-4-20250514".to_string(),
            })
    }

    /// Get base URL, with fallback to environment variable.
    /// Universal env var: BASE_URL (also supports ANTHROPIC_BASE_URL for compatibility)
    pub fn get_base_url(&self, provider: &str) -> String {
        env::var("BASE_URL").ok()
            .or_else(|| env::var("ANTHROPIC_BASE_URL").ok())
            .or(self.base_url.clone())
            .unwrap_or_else(|| match provider {
                "openai" => "https://api.openai.com/v1".to_string(),
                _ => "https://api.anthropic.com".to_string(),
            })
    }

    /// Save configuration to ~/.matrix/config.json.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::matrix_config_path()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

        // Create directory if needed
        let dir = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        println!("[config saved to ~/.matrix/config.json]");
        Ok(())
    }

    /// Check if API is configured.
    pub fn is_api_configured(&self) -> bool {
        self.api_key.is_some()
            || env::var("API_KEY").ok().is_some()
            || env::var("ANTHROPIC_AUTH_TOKEN").ok().is_some()
    }
}

/// Create a default config file for new users.
pub fn create_default_config() -> anyhow::Result<()> {
    let config = MatrixConfig {
        provider: Some("anthropic".to_string()),
        api_key: None,
        base_url: None,
        model: None,
        think: true,
        markdown: true,
        max_tokens: 16384,
        context_size: None,
        multi_model: Some(false),
        plan_model: None,
        compress_model: None,
        fast_model: None,
        approve_mode: Some("ask".to_string()),
        extra_headers: None,
    };

    config.save()?;
    println!("\nConfig file created at ~/.matrix/config.json");
    println!("Universal field names:");
    println!("  api_key        - API key");
    println!("  base_url       - API endpoint");
    println!("  model          - Main model");
    println!("  plan_model     - Planning model");
    println!("  compress_model - Compression model");
    println!("  extra_headers  - Custom HTTP headers");
    println!("\nEnvironment variables:");
    println!("  API_KEY, BASE_URL, MODEL");
    println!("\nLegacy aliases (still supported):");
    println!("  ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, ANTHROPIC_MODEL");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = MatrixConfig {
            provider: None,
            api_key: None,
            base_url: None,
            model: None,
            think: true,
            markdown: true,
            max_tokens: 16384,
            context_size: None,
            multi_model: None,
            plan_model: None,
            compress_model: None,
            fast_model: None,
            approve_mode: None,
        };
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
        assert!(config.think);
        assert!(config.markdown);
        assert_eq!(config.max_tokens, 16384);
    }

    #[test]
    fn test_universal_field_names() {
        // Universal naming
        let json = r#"{
            "api_key": "test-key",
            "base_url": "https://test.com",
            "model": "test-model",
            "plan_model": "reasoning-model",
            "compress_model": "haiku-model"
        }"#;

        let config: MatrixConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.base_url, Some("https://test.com".to_string()));
        assert_eq!(config.model, Some("test-model".to_string()));
        assert_eq!(config.plan_model, Some("reasoning-model".to_string()));
        assert_eq!(config.compress_model, Some("haiku-model".to_string()));
    }

    #[test]
    fn test_legacy_alias_names() {
        // Legacy ANTHROPIC_ prefixed names (still supported via alias)
        let json = r#"{
            "ANTHROPIC_AUTH_TOKEN": "test-key",
            "ANTHROPIC_BASE_URL": "https://test.com",
            "ANTHROPIC_MODEL": "test-model",
            "ANTHROPIC_REASONING_MODEL": "reasoning-model",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": "haiku-model"
        }"#;

        let config: MatrixConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.base_url, Some("https://test.com".to_string()));
        assert_eq!(config.model, Some("test-model".to_string()));
        assert_eq!(config.plan_model, Some("reasoning-model".to_string()));
        assert_eq!(config.compress_model, Some("haiku-model".to_string()));
    }

    #[test]
    fn test_serialization_uses_universal_names() {
        let config = MatrixConfig {
            api_key: Some("key".to_string()),
            model: Some("model".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        // Should use universal field names
        assert!(json.contains("api_key"));
        assert!(json.contains("model"));
    }
}
