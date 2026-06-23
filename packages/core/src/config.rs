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
//! Priority (highest to lowest):
//! 1. Environment variables (API_KEY, BASE_URL, MODEL, etc.)
//! 2. ~/.matrix/config.json (matrixcode's own config)
//! 3. ~/.claude/settings.json (Claude Code fallback)
//! 4. Defaults

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::constants::{DEFAULT_MAX_TOKENS, ANTHROPIC_DEFAULT_BASE_URL, OPENAI_DEFAULT_BASE_URL, MATRIX_DIR};
use crate::models::DEFAULT_MAIN_MODEL;

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

    /// Enable LSP integration
    #[serde(default)]
    pub enable_lsp: bool,

    /// Verification strategy for code changes
    #[serde(default)]
    pub verify_strategy: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_max_tokens() -> u32 {
    DEFAULT_MAX_TOKENS
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
        Self::home_dir().map(|h| h.join(MATRIX_DIR).join("config.json"))
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
            max_tokens: DEFAULT_MAX_TOKENS,
            context_size: None,
            multi_model: None,
            plan_model: env.ANTHROPIC_REASONING_MODEL,
            compress_model: env.ANTHROPIC_DEFAULT_HAIKU_MODEL,
            fast_model: None,
            approve_mode: Some("ask".to_string()),
            extra_headers: None,
            enable_lsp: false,
            verify_strategy: None,
        })
    }

    /// Load configuration from environment variables.
    /// Universal env vars: API_KEY, BASE_URL, MODEL
    /// Also supports legacy: ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, ANTHROPIC_MODEL
    fn load_from_env() -> Self {
        // Parse EXTRA_HEADERS from env if available (JSON format)
        let extra_headers = env::var("EXTRA_HEADERS").ok()
            .and_then(|json_str| serde_json::from_str::<HashMap<String, String>>(&json_str).ok());

        Self {
            provider: env::var("PROVIDER").ok(),
            api_key: env::var("API_KEY").ok()
                .or_else(|| env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .or_else(|| env::var("ANTHROPIC_API_KEY").ok()),
            base_url: env::var("BASE_URL").ok()
                .or_else(|| env::var("ANTHROPIC_BASE_URL").ok()),
            model: env::var("MODEL").ok()
                .or_else(|| env::var("ANTHROPIC_MODEL").ok())
                .or_else(|| env::var("MODEL_NAME").ok()),
            think: env::var("THINK").ok()
                .map(|v| v != "false")
                .unwrap_or(true),
            markdown: env::var("MARKDOWN").ok()
                .map(|v| v != "false")
                .unwrap_or(true),
            max_tokens: env::var("MAX_TOKENS").ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_TOKENS),
            context_size: env::var("CONTEXT_SIZE").ok()
                .and_then(|v| v.parse().ok()),
            multi_model: env::var("MULTI_MODEL").ok()
                .map(|v| v == "true"),
            plan_model: env::var("ANTHROPIC_REASONING_MODEL").ok(),
            compress_model: env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok(),
            fast_model: None,
            approve_mode: env::var("APPROVE_MODE").ok()
                .or(Some("ask".to_string())),
            extra_headers,
            enable_lsp: env::var("ENABLE_LSP").ok()
                .map(|v| v == "true")
                .unwrap_or(false),
            verify_strategy: env::var("VERIFY_STRATEGY").ok(),
        }
    }

    /// Load configuration with fallback chain.
    /// Priority: env vars > ~/.matrix/config.json > ~/.claude/settings.json > defaults
    pub fn load() -> Self {
        // Load all sources
        let env_config = Self::load_from_env();
        let matrix_config = Self::load_matrix_config();
        let claude_config = Self::load_claude_settings();

        // Auto-create example config if neither config file exists
        if matrix_config.is_none() && claude_config.is_none() && env_config.api_key.is_none() {
            let _ = create_example_config();
            println!("[config: No config found. Example created at ~/.matrix/config.example.json]");
            println!("\nTo configure, create ~/.matrix/config.json with:");
            println!("  {{");
            println!("    \"provider\": \"anthropic\",");
            println!("    \"api_key\": \"your-api-key\",");
            println!("    \"model\": \"claude-sonnet-4-20250514\"");
            println!("  }}\n");
        }

        // Determine which sources are active
        let has_env = env_config.api_key.is_some() || env_config.model.is_some();
        let has_matrix = matrix_config.is_some();
        let has_claude = claude_config.is_some();

        // Build source description
        let sources: Vec<&str> = [
            has_env.then_some("env"),
            has_matrix.then_some("~/.matrix/config.json"),
            has_claude.then_some("~/.claude/settings.json"),
        ].iter().flatten().copied().collect();
        println!("[config: {}]", sources.join(" + "));

        // Merge with correct priority: env > matrix > claude > defaults
        // Start with defaults, then layer on configs in reverse priority order
        let mut merged = Self::default();

        // Claude config (lowest priority, fills in missing fields)
        if let Some(cc) = claude_config {
            merged.provider = merged.provider.or(cc.provider);
            merged.api_key = merged.api_key.or(cc.api_key);
            merged.base_url = merged.base_url.or(cc.base_url);
            merged.model = merged.model.or(cc.model);
            merged.think = cc.think; // Default from claude
            merged.markdown = cc.markdown;
            merged.max_tokens = cc.max_tokens;
            merged.context_size = merged.context_size.or(cc.context_size);
            merged.multi_model = merged.multi_model.or(cc.multi_model);
            merged.plan_model = merged.plan_model.or(cc.plan_model);
            merged.compress_model = merged.compress_model.or(cc.compress_model);
            merged.fast_model = merged.fast_model.or(cc.fast_model);
            merged.approve_mode = merged.approve_mode.or(cc.approve_mode);
            merged.extra_headers = merged.extra_headers.or(cc.extra_headers);
        }

        // Matrix config (medium priority, overrides claude)
        if let Some(mx) = matrix_config {
            merged.provider = merged.provider.or(mx.provider);
            merged.api_key = merged.api_key.or(mx.api_key);
            merged.base_url = merged.base_url.or(mx.base_url);
            merged.model = merged.model.or(mx.model);
            merged.think = mx.think;
            merged.markdown = mx.markdown;
            merged.max_tokens = mx.max_tokens;
            merged.context_size = merged.context_size.or(mx.context_size);
            merged.multi_model = merged.multi_model.or(mx.multi_model);
            merged.plan_model = merged.plan_model.or(mx.plan_model);
            merged.compress_model = merged.compress_model.or(mx.compress_model);
            merged.fast_model = merged.fast_model.or(mx.fast_model);
            merged.approve_mode = merged.approve_mode.or(mx.approve_mode);
            merged.extra_headers = merged.extra_headers.or(mx.extra_headers);
        }

        // Env config (highest priority, overrides everything)
        merged.provider = env_config.provider.or(merged.provider);
        merged.api_key = env_config.api_key.or(merged.api_key);
        merged.base_url = env_config.base_url.or(merged.base_url);
        merged.model = env_config.model.or(merged.model);
        merged.think = env_config.think;
        merged.markdown = env_config.markdown;
        merged.max_tokens = env_config.max_tokens;
        merged.context_size = env_config.context_size.or(merged.context_size);
        merged.multi_model = env_config.multi_model.or(merged.multi_model);
        merged.plan_model = env_config.plan_model.or(merged.plan_model);
        merged.compress_model = env_config.compress_model.or(merged.compress_model);
        merged.fast_model = env_config.fast_model.or(merged.fast_model);
        merged.approve_mode = env_config.approve_mode.or(merged.approve_mode);
        merged.extra_headers = env_config.extra_headers.or(merged.extra_headers);

        // Ensure approve_mode has a default
        merged.approve_mode = merged.approve_mode.or(Some("ask".to_string()));

        merged
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
                _ => DEFAULT_MAIN_MODEL.to_string(),
            })
    }

    /// Get base URL, with fallback to environment variable.
    /// Universal env var: BASE_URL (also supports ANTHROPIC_BASE_URL for compatibility)
    pub fn get_base_url(&self, provider: &str) -> String {
        env::var("BASE_URL").ok()
            .or_else(|| env::var("ANTHROPIC_BASE_URL").ok())
            .or(self.base_url.clone())
            .unwrap_or_else(|| match provider {
                "openai" => OPENAI_DEFAULT_BASE_URL.to_string(),
                _ => ANTHROPIC_DEFAULT_BASE_URL.to_string(),
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

    /// Get API key with fallback chain
    pub fn resolve_api_key(&self) -> Option<String> {
        self.api_key.clone()
            .or_else(|| env::var("ANTHROPIC_AUTH_TOKEN").ok())
            .or_else(|| env::var("API_KEY").ok())
    }

    /// Get model with fallback chain
    pub fn resolve_model(&self) -> String {
        self.model.clone()
            .or_else(|| env::var("MODEL").ok())
            .or_else(|| env::var("ANTHROPIC_MODEL").ok())
            .unwrap_or_else(|| DEFAULT_MAIN_MODEL.to_string())
    }

    /// Get base URL with fallback chain
    pub fn resolve_base_url(&self) -> Option<String> {
        self.base_url.clone()
            .or_else(|| env::var("BASE_URL").ok())
            .or_else(|| env::var("ANTHROPIC_BASE_URL").ok())
    }

    /// Infer provider type from model name
    fn infer_provider_type(model: &str) -> crate::providers::ProviderType {
        if model.starts_with("gpt") || model.starts_with("o1") {
            crate::providers::ProviderType::OpenAI
        } else {
            crate::providers::ProviderType::Anthropic
        }
    }

    /// Resolve provider type from config or infer from model
    pub fn resolve_provider_type(&self, model: &str) -> crate::providers::ProviderType {
        use crate::providers::ProviderType;

        self.provider.clone()
            .or_else(|| env::var("PROVIDER").ok())
            .map(|p| match p.to_lowercase().as_str() {
                "openai" => ProviderType::OpenAI,
                _ => ProviderType::Anthropic,
            })
            .unwrap_or_else(|| Self::infer_provider_type(model))
    }

    /// Create a Provider instance from configuration.
    /// Useful for tools that need AI capabilities but don't have an injected provider.
    pub fn create_provider_from_env() -> anyhow::Result<std::sync::Arc<dyn crate::providers::Provider>> {
        let config = Self::load();

        let api_key = config.resolve_api_key()
            .ok_or_else(|| anyhow::anyhow!("未配置 API key，无法执行 AI 任务"))?;

        let model = config.resolve_model();
        let provider_type = config.resolve_provider_type(&model);
        let base_url = config.resolve_base_url();

        crate::providers::create_provider_with_headers(
            provider_type,
            api_key,
            model,
            base_url,
            config.extra_headers.clone()
        ).map(std::sync::Arc::from)
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
        max_tokens: DEFAULT_MAX_TOKENS,
        context_size: None,
        multi_model: Some(false),
        plan_model: None,
        compress_model: None,
        fast_model: None,
        approve_mode: Some("ask".to_string()),
        extra_headers: None,
        enable_lsp: false,
        verify_strategy: None,
    };

    config.save()?;

    // Also create example config with documentation
    create_example_config()?;

    println!("\nConfig file created at ~/.matrix/config.json");
    println!("Example config with documentation: ~/.matrix/config.example.json");
    println!("\nRequired fields to fill:");
    println!("  api_key  - Your API key");
    println!("  model    - Model name (e.g. claude-sonnet-4-20250514, gpt-4o, glm-5)");
    println!("\nOptional fields:");
    println!("  provider   - 'anthropic' or 'openai' (auto-detected from model if not set)");
    println!("  base_url   - API endpoint (uses default if not set)");
    println!("  extra_headers - Custom HTTP headers for API requests");
    Ok(())
}

/// Create example config file with field documentation.
pub fn create_example_config() -> anyhow::Result<()> {
    let home = MatrixConfig::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let path = home.join(MATRIX_DIR).join("config.example.json");

    let example = r#"{
  "_comment": "MatrixCode Configuration Example - Copy this to config.json and fill in your values",

  "provider": "anthropic",
  "_provider_comment": "API provider: 'anthropic' or 'openai'. Auto-detected from model name if not set.",

  "api_key": "your-api-key-here",
  "_api_key_comment": "Your API key. Also supports env vars: API_KEY, ANTHROPIC_AUTH_TOKEN, OPENAI_API_KEY",

  "model": "claude-sonnet-4-20250514",
  "_model_comment": "Model name. Examples: claude-sonnet-4, claude-opus-4, gpt-4o, glm-5",

  "base_url": null,
  "_base_url_comment": "API endpoint. Defaults: anthropic=https://api.anthropic.com, openai=https://api.openai.com/v1",
  "_base_url_examples": ["https://dashscope.aliyuncs.com/compatible-mode/v1 for DashScope"],

  "think": true,
  "_think_comment": "Enable extended thinking (Anthropic only). Set false for non-Anthropic endpoints.",

  "markdown": true,
  "_markdown_comment": "Enable markdown rendering in TUI",

  "max_tokens": 16384,
  "_max_tokens_comment": "Maximum output tokens per request",

  "approve_mode": "ask",
  "_approve_mode_comment": "Tool approval: 'ask'=prompt each, 'auto'=approve safe, 'strict'=reject dangerous",

  "multi_model": false,
  "_multi_model_comment": "Enable multi-model configuration",

  "plan_model": null,
  "_plan_model_comment": "Planning/reasoning model for complex tasks",

  "compress_model": null,
  "_compress_model_comment": "Fast model for context compression",

  "fast_model": null,
  "_fast_model_comment": "Fast model for quick operations",

  "extra_headers": {},
  "_extra_headers_comment": "Custom HTTP headers for API requests (useful for proxy services)",
  "_extra_headers_example": {"X-DashScope-SSE": "enable"}
}"#;

    std::fs::write(&path, example)?;
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
            max_tokens: DEFAULT_MAX_TOKENS,
            context_size: None,
            multi_model: None,
            plan_model: None,
            compress_model: None,
            fast_model: None,
            approve_mode: None,
            extra_headers: None,
            enable_lsp: false,
            verify_strategy: None,
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
            extra_headers: None,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        // Should use universal field names
        assert!(json.contains("api_key"));
        assert!(json.contains("model"));
    }
}
