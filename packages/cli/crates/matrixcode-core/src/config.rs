//! Agent Configuration

use serde::{Deserialize, Serialize};
use crate::approval::ApproveMode;

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// System prompt
    pub system_prompt: String,
    /// Max output tokens
    pub max_tokens: u32,
    /// Enable thinking mode
    pub think: bool,
    /// Approval mode
    pub approve_mode: ApproveMode,
    /// Model name
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system_prompt: "You are a helpful AI assistant.".to_string(),
            max_tokens: 4096,
            think: false,
            approve_mode: ApproveMode::Ask,
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }
}

impl Config {
    /// Load from environment
    pub fn from_env() -> Self {
        Self {
            system_prompt: std::env::var("SYSTEM_PROMPT")
                .unwrap_or_else(|_| "You are a helpful AI assistant.".to_string()),
            max_tokens: std::env::var("MAX_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4096),
            think: std::env::var("THINK")
                .ok()
                .map(|s| s == "true")
                .unwrap_or(false),
            approve_mode: ApproveMode::from_str(
                &std::env::var("APPROVE_MODE").unwrap_or_else(|_| "ask".to_string())
            ),
            model: std::env::var("MODEL_NAME")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
        }
    }
}