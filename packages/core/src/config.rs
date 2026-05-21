//! Configuration loading for matrixcode.
//!
//! Variable names are aligned with Claude Code for consistency:
//! - ANTHROPIC_AUTH_TOKEN (API key)
//! - ANTHROPIC_BASE_URL
//! - ANTHROPIC_MODEL
//! - ANTHROPIC_DEFAULT_SONNET_MODEL
//! - ANTHROPIC_DEFAULT_HAIKU_MODEL (compress model)
//! - ANTHROPIC_REASONING_MODEL (plan model)
//!
//! Priority:
//! 1. CLI arguments (highest priority)
//! 2. ~/.matrix/config.json (matrixcode's own config)
//! 3. ~/.claude/settings.json (Claude Code config)
//! 4. Environment variables
//!
//! This allows seamless sharing of settings between matrixcode and Claude Code.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// Matrixcode configuration file structure.
/// Field names align with Claude Code conventions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixConfig {
    /// LLM provider: "anthropic" or "openai"
    #[serde(default)]
    pub provider: Option<String>,

    /// API key (Claude Code style: ANTHROPIC_AUTH_TOKEN)
    #[serde(default, rename = "ANTHROPIC_AUTH_TOKEN")]
    pub api_key: Option<String>,

    /// Base URL for API endpoint
    #[serde(default, rename = "ANTHROPIC_BASE_URL")]
    pub base_url: Option<String>,

    /// Main model name
    #[serde(default, rename = "ANTHROPIC_MODEL")]
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

    /// Plan/reasoning model (Claude Code style: ANTHROPIC_REASONING_MODEL)
    #[serde(default, rename = "ANTHROPIC_REASONING_MODEL")]
    pub plan_model: Option<String>,

    /// Compress/haiku model (Claude Code style: ANTHROPIC_DEFAULT_HAIKU_MODEL)
    #[serde(default, rename = "ANTHROPIC_DEFAULT_HAIKU_MODEL")]
    pub compress_model: Option<String>,

    /// Fast model
    #[serde(default)]
    pub fast_model: Option<String>,

    /// Approve mode: "ask", "auto", "strict"
    #[serde(default = "default_approve_mode")]
    pub approve_mode: Option<String>,
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

/// Claude Code settings.json structure.
#[derive(Debug, Clone, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    env: Option<ClaudeEnv>,

    /// If true, skip dangerous mode permission prompts -> approve_mode = "auto"
    #[serde(default, rename = "skipDangerousModePermissionPrompt")]
    skip_dangerous_mode_permission_prompt: Option<bool>,
}

/// Environment variables from Claude Code settings.
/// Uses SCREAMING_SNAKE_CASE to match Claude Code convention.
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

    /// Path to cc-switch settings file.
    pub fn claude_settings_path() -> Option<PathBuf> {
        Self::home_dir().map(|h| h.join(".claude").join("settings.json"))
    }

    /// Load matrixcode's own config file.
    fn load_matrix_config() -> Option<Self> {
        let path = Self::matrix_config_path()?;
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let config: Self = serde_json::from_str(&content).ok()?;

        // Don't print here - we'll print after merge
        Some(config)
    }

    /// Load Claude Code settings and convert to matrixcode config.
    fn load_ccswitch_config() -> Option<Self> {
        let path = Self::claude_settings_path()?;
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let settings: ClaudeSettings = serde_json::from_str(&content).ok()?;

        let env = settings.env?;

        // Convert skip_dangerous_mode_permission_prompt to approve_mode
        let approve_mode = if settings.skip_dangerous_mode_permission_prompt == Some(true) {
            Some("auto".to_string())
        } else {
            None
        };

        // Convert Claude Code env to matrixcode config (same field names now)
        let config = Self {
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
            approve_mode,
        };

        Some(config)
    }

    /// Load configuration with fallback chain.
    /// Priority: CLI args > ~/.matrix/config.json > ~/.claude/settings.json > env vars
    ///
    /// Fields are merged: matrix config values take precedence, missing fields
    /// fall back to Claude settings, then to defaults/env vars.
    pub fn load() -> Self {
        // Try matrixcode's own config first
        let matrix_config = Self::load_matrix_config();
        // Load Claude settings as fallback source
        let claude_config = Self::load_ccswitch_config();

        // Merge: matrix config takes precedence, fallback to Claude for missing fields
        match (matrix_config, claude_config) {
            (Some(mx), Some(cc)) => {
                // Check if we need fallback
                let needs_fallback =
                    mx.api_key.is_none() || mx.model.is_none() || mx.base_url.is_none();

                // Merge: matrix values take precedence
                // For approve_mode: use matrix config, or default to "ask" (not Claude's auto)
                let approve_mode = mx.approve_mode.or(Some("ask".to_string()));

                let merged = Self {
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
                    approve_mode,
                };

                // Show which config source(s) are being used
                if needs_fallback {
                    println!(
                        "[config: ~/.matrix/config.json + fallback from ~/.claude/settings.json]"
                    );
                } else {
                    println!("[config: ~/.matrix/config.json]");
                }
                merged
            }
            (Some(mx), None) => {
                println!("[config: ~/.matrix/config.json]");
                // Ensure approve_mode has default
                if mx.approve_mode.is_none() {
                    Self {
                        approve_mode: Some("ask".to_string()),
                        ..mx
                    }
                } else {
                    mx
                }
            }
            (None, Some(cc)) => {
                println!("[config: ~/.claude/settings.json (Claude Code)]");
                // Override Claude's approve_mode to "ask" by default
                // MatrixCode defaults to ask mode for safety
                Self {
                    approve_mode: Some("ask".to_string()),
                    ..cc
                }
            }
            (None, None) => {
                println!("[config: using defaults and environment variables]");
                Self::default()
            }
        }
    }

    /// Get API key, with fallback to environment variable.
    /// Uses Claude Code style: ANTHROPIC_AUTH_TOKEN
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => env::var("OPENAI_API_KEY").ok(),
            _ => env::var("ANTHROPIC_AUTH_TOKEN")
                .or_else(|_| env::var("ANTHROPIC_API_KEY")) // fallback for compatibility
                .ok(),
        }
        .or(self.api_key.clone())
    }

    /// Get model name, with fallback to environment variable.
    /// Uses Claude Code style: ANTHROPIC_MODEL
    pub fn get_model(&self, provider: &str) -> String {
        env::var("ANTHROPIC_MODEL")
            .or_else(|_| env::var("MODEL_NAME")) // fallback for compatibility
            .ok()
            .or(self.model.clone())
            .unwrap_or_else(|| match provider {
                "openai" => "gpt-4o".to_string(),
                _ => "claude-sonnet-4-20250514".to_string(),
            })
    }

    /// Get base URL, with fallback to environment variable.
    /// Uses Claude Code style: ANTHROPIC_BASE_URL
    pub fn get_base_url(&self, provider: &str) -> String {
        env::var("ANTHROPIC_BASE_URL")
            .or_else(|_| env::var("BASE_URL")) // fallback for compatibility
            .ok()
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
}

/// Create a default config file for new users.
/// Uses Claude Code style field names.
pub fn create_default_config() -> anyhow::Result<()> {
    let config = MatrixConfig {
        provider: Some("anthropic".to_string()),
        api_key: None,  // ANTHROPIC_AUTH_TOKEN - user should fill
        base_url: None, // ANTHROPIC_BASE_URL
        model: None,    // ANTHROPIC_MODEL - will fallback to Claude settings
        think: true,
        markdown: true,
        max_tokens: 16384,
        context_size: None,
        multi_model: Some(false),
        plan_model: None,     // ANTHROPIC_REASONING_MODEL
        compress_model: None, // ANTHROPIC_DEFAULT_HAIKU_MODEL
        fast_model: None,
        approve_mode: Some("ask".to_string()),
    };

    config.save()?;
    println!("\nConfig file created at ~/.matrix/config.json");
    println!("Fields use Claude Code naming convention:");
    println!("  ANTHROPIC_AUTH_TOKEN      - API key");
    println!("  ANTHROPIC_BASE_URL        - API endpoint");
    println!("  ANTHROPIC_MODEL           - Main model");
    println!("  ANTHROPIC_REASONING_MODEL - Planning model");
    println!("  ANTHROPIC_DEFAULT_HAIKU_MODEL - Compression model");
    println!("\nLeave fields as null to fallback to ~/.claude/settings.json");
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
    fn test_claude_code_field_names() {
        // Verify serde renames work correctly
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
    fn test_serialization_uses_claude_names() {
        let config = MatrixConfig {
            api_key: Some("key".to_string()),
            model: Some("model".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        // Should use Claude Code field names
        assert!(json.contains("ANTHROPIC_AUTH_TOKEN"));
        assert!(json.contains("ANTHROPIC_MODEL"));
    }
}
