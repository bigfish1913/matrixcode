//! Configuration loading for matrixcode.
//!
//! Priority:
//! 1. CLI arguments (highest priority)
//! 2. ~/.matrix/config.json (matrixcode's own config)
//! 3. ~/.claude/settings.json (cc-switch config)
//! 4. Environment variables (.env file loaded by dotenvy)
//!
//! This allows users to share API keys and settings between
//! matrixcode and cc-switch (Claude Code).

use std::path::PathBuf;
use std::env;
use serde::{Deserialize, Serialize};

/// Matrixcode configuration file structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixConfig {
    /// LLM provider: "anthropic" or "openai"
    #[serde(default)]
    pub provider: Option<String>,
    
    /// API key for the provider
    #[serde(default)]
    pub api_key: Option<String>,
    
    /// Base URL for API endpoint
    #[serde(default)]
    pub base_url: Option<String>,
    
    /// Model name
    #[serde(default)]
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
    
    /// Plan model name
    #[serde(default)]
    pub plan_model: Option<String>,
    
    /// Compress model name
    #[serde(default)]
    pub compress_model: Option<String>,
    
    /// Fast model name
    #[serde(default)]
    pub fast_model: Option<String>,
    
    /// Approve mode: "ask", "auto", "strict"
    #[serde(default)]
    pub approve_mode: Option<String>,
}

fn default_true() -> bool { true }
fn default_max_tokens() -> u32 { 16384 }

/// cc-switch (Claude Code) settings.json structure.
/// We only extract relevant env variables.
#[derive(Debug, Clone, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    env: Option<ClaudeEnv>,
}

/// Environment variables from cc-switch settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct ClaudeEnv {
    #[serde(default)]
    anthropic_auth_token: Option<String>,
    
    #[serde(default)]
    anthropic_base_url: Option<String>,
    
    #[serde(default)]
    anthropic_model: Option<String>,
    
    #[serde(rename = "ANTHROPIC_DEFAULT_HAIKU_MODEL")]
    #[serde(default)]
    pub compress_model: Option<String>,
    
    #[serde(rename = "ANTHROPIC_REASONING_MODEL")]
    #[serde(default)]
    pub plan_model: Option<String>,
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
        
        if config.api_key.is_some() || config.model.is_some() {
            println!("[loaded config from ~/.matrix/config.json]");
        }
        
        Some(config)
    }
    
    /// Load cc-switch settings and convert to matrixcode config.
    fn load_ccswitch_config() -> Option<Self> {
        let path = Self::claude_settings_path()?;
        if !path.exists() {
            return None;
        }
        
        let content = std::fs::read_to_string(&path).ok()?;
        let settings: ClaudeSettings = serde_json::from_str(&content).ok()?;
        
        let env = settings.env?;
        
        // Convert cc-switch env to matrixcode config
        let config = Self {
            provider: Some("anthropic".to_string()),
            api_key: env.anthropic_auth_token,
            base_url: env.anthropic_base_url,
            model: env.anthropic_model,
            think: true,
            markdown: true,
            max_tokens: 16384,
            context_size: None,
            multi_model: None,
            plan_model: env.plan_model,
            compress_model: env.compress_model,
            fast_model: None,
            approve_mode: None,
        };
        
        if config.api_key.is_some() {
            println!("[loaded config from ~/.claude/settings.json (cc-switch)]");
        }
        
        Some(config)
    }
    
    /// Load configuration with fallback chain.
    /// Priority: CLI args > ~/.matrix/config.json > ~/.claude/settings.json > env vars
    pub fn load() -> Self {
        // Try matrixcode's own config first
        Self::load_matrix_config()
            // Fallback to cc-switch config
            .or_else(Self::load_ccswitch_config)
            // Default if no config files found
            .unwrap_or_default()
    }
    
    /// Get API key, with fallback to environment variable.
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        // Check environment variable first (dotenvy already loaded .env)
        match provider {
            "openai" => env::var("OPENAI_API_KEY").ok(),
            _ => env::var("ANTHROPIC_API_KEY")
                .or_else(|_| env::var("API_KEY"))
                .ok(),
        }
        // Then use config file value if env var not set
        .or(self.api_key.clone())
    }
    
    /// Get model name, with fallback to default.
    pub fn get_model(&self, provider: &str) -> String {
        // Check environment variable
        env::var("MODEL_NAME")
            .ok()
            // Then config file
            .or(self.model.clone())
            // Then provider default
            .unwrap_or_else(|| match provider {
                "openai" => "gpt-4o".to_string(),
                _ => "claude-sonnet-4-20250514".to_string(),
            })
    }
    
    /// Get base URL, with fallback to default.
    pub fn get_base_url(&self, provider: &str) -> String {
        env::var("BASE_URL")
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
        let dir = path.parent().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
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
pub fn create_default_config() -> anyhow::Result<()> {
    let config = MatrixConfig {
        provider: Some("anthropic".to_string()),
        api_key: None,  // User should fill this
        base_url: None,
        model: Some("claude-sonnet-4-20250514".to_string()),
        think: true,
        markdown: true,
        max_tokens: 16384,
        context_size: None,
        multi_model: Some(false),
        plan_model: None,
        compress_model: None,
        fast_model: None,
        approve_mode: Some("ask".to_string()),
    };
    
    config.save()?;
    println!("\nEdit ~/.matrix/config.json to set your API key.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = MatrixConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
        assert!(config.think);
        assert!(config.markdown);
        assert_eq!(config.max_tokens, 16384);
    }
    
    #[test]
    fn test_model_fallback() {
        let config = MatrixConfig::default();
        let model = config.get_model("anthropic");
        assert_eq!(model, "claude-sonnet-4-20250514");
        
        let model = config.get_model("openai");
        assert_eq!(model, "gpt-4o");
    }
}