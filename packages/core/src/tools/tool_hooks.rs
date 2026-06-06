//! Tool Execution Hooks System
//!
//! This module provides a hook system for tool execution, allowing
//! pre and post execution checks and modifications.
//!
//! # Hook Types
//!
//! - `PreExecute`: Before tool execution, can block or modify params
//! - `PostExecute`: After tool execution, can modify result
//!
//! # Usage
//!
//! ```rust
//! // Create a hook registry
//! let registry = HookRegistry::new();
//!
//! // Register hooks
//! registry.register(Box::new(CodeQualityHook::new("pre")));
//!
//! // Execute with hooks
//! let result = registry.execute_with_hooks(tool, params);
//! ```

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Hook execution result
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue with original params
    Continue,
    /// Block execution with reason (returned to AI for correction)
    Block {
        reason: String,
        /// Detailed error information for AI to correct
        details: Option<String>,
    },
    /// Modify params before execution
    Modify(Value),
}

/// Tool execution hook trait
#[async_trait]
pub trait ToolHook: Send + Sync {
    /// Hook name for identification
    fn name(&self) -> &str;

    /// Whether this hook is enabled
    fn is_enabled(&self) -> bool;

    /// Tools this hook applies to (empty = all tools)
    fn applies_to(&self) -> Vec<&str> {
        Vec::new()
    }

    /// Check if hook applies to a specific tool
    fn applies_to_tool(&self, tool_name: &str) -> bool {
        let applies_to = self.applies_to();
        applies_to.is_empty() || applies_to.iter().any(|t| *t == tool_name)
    }

    /// Pre-execute hook (before tool execution)
    /// Returns HookResult to control execution flow
    async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult>;

    /// Post-execute hook (after tool execution)
    /// Can modify the result before returning to AI
    async fn post_execute(&self, tool_name: &str, params: &Value, result: &str) -> Result<String>;
}

/// Hook registry for managing multiple hooks
pub struct HookRegistry {
    hooks: Vec<Box<dyn ToolHook>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Create registry with default hooks
    pub fn with_defaults() -> Self {
        Self::new()
    }

    /// Register a hook
    pub fn register(&mut self, hook: Box<dyn ToolHook>) {
        self.hooks.push(hook);
    }

    /// Get all registered hooks
    pub fn hooks(&self) -> &[Box<dyn ToolHook>] {
        &self.hooks
    }

    /// Run pre-execute hooks for a tool
    /// Returns first Block result, or Continue/Modify from last hook
    pub async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult> {
        let mut current_params = params.clone();

        for hook in &self.hooks {
            if !hook.is_enabled() || !hook.applies_to_tool(tool_name) {
                continue;
            }

            let result = hook.pre_execute(tool_name, &current_params).await?;

            match result {
                HookResult::Block { .. } => {
                    // Block immediately, return to AI
                    return Ok(result);
                }
                HookResult::Modify(new_params) => {
                    // Update params for next hook
                    current_params = new_params;
                }
                HookResult::Continue => {
                    // Continue to next hook
                }
            }
        }

        // If params were modified, return Modify; otherwise Continue
        if current_params != *params {
            Ok(HookResult::Modify(current_params))
        } else {
            Ok(HookResult::Continue)
        }
    }

    /// Run post-execute hooks for a tool
    /// Returns modified result after all hooks
    pub async fn post_execute(&self, tool_name: &str, params: &Value, result: &str) -> Result<String> {
        let mut current_result = result.to_string();

        for hook in &self.hooks {
            if !hook.is_enabled() || !hook.applies_to_tool(tool_name) {
                continue;
            }

            current_result = hook.post_execute(tool_name, params, &current_result).await?;
        }

        Ok(current_result)
    }
}

/// Global hook registry instance
static GLOBAL_HOOK_REGISTRY: std::sync::OnceLock<Arc<HookRegistry>> = std::sync::OnceLock::new();

/// Get the global hook registry
pub fn global_hook_registry() -> Arc<HookRegistry> {
    GLOBAL_HOOK_REGISTRY
        .get_or_init(|| Arc::new(HookRegistry::with_defaults()))
        .clone()
}

/// Set the global hook registry
pub fn set_global_hook_registry(registry: HookRegistry) {
    let _ = GLOBAL_HOOK_REGISTRY.set(Arc::new(registry));
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHook {
        enabled: bool,
        block: bool,
    }

    #[async_trait]
    impl ToolHook for TestHook {
        fn name(&self) -> &str {
            "test_hook"
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn applies_to(&self) -> Vec<&str> {
            vec!["write"]
        }

        async fn pre_execute(&self, _tool_name: &str, _params: &Value) -> Result<HookResult> {
            if self.block {
                Ok(HookResult::Block {
                    reason: "Test block".to_string(),
                    details: Some("Test details".to_string()),
                })
            } else {
                Ok(HookResult::Continue)
            }
        }

        async fn post_execute(&self, _tool_name: &str, _params: &Value, result: &str) -> Result<String> {
            Ok(format!("{} [hooked]", result))
        }
    }

    #[tokio::test]
    async fn test_hook_registry_pre_execute_continue() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(TestHook { enabled: true, block: false }));

        let result = registry.pre_execute("write", &serde_json::json!({})).await;
        assert!(matches!(result.unwrap(), HookResult::Continue));
    }

    #[tokio::test]
    async fn test_hook_registry_pre_execute_block() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(TestHook { enabled: true, block: true }));

        let result = registry.pre_execute("write", &serde_json::json!({})).await;
        assert!(matches!(result.unwrap(), HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_disabled_hook() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(TestHook { enabled: false, block: true }));

        let result = registry.pre_execute("write", &serde_json::json!({})).await;
        assert!(matches!(result.unwrap(), HookResult::Continue));
    }

    #[tokio::test]
    async fn test_hook_registry_tool_filter() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(TestHook { enabled: true, block: true }));

        // Should not block for tools not in applies_to
        let result = registry.pre_execute("read", &serde_json::json!({})).await;
        assert!(matches!(result.unwrap(), HookResult::Continue));

        // Should block for tools in applies_to
        let result = registry.pre_execute("write", &serde_json::json!({})).await;
        assert!(matches!(result.unwrap(), HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_post_execute() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(TestHook { enabled: true, block: false }));

        let result = registry.post_execute("write", &serde_json::json!({}), "original").await;
        assert_eq!(result.unwrap(), "original [hooked]");
    }
}