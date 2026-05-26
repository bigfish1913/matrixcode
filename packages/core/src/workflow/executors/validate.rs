//! Validate Executor
//!
//! 混合验证执行器，程序规则验证 + AI 验证。

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::providers::{ChatRequest, ContentBlock, Message, MessageContent, Provider};
use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;
use crate::workflow::rule_engine::{Rule, RuleEngine, ValidationResult};
use crate::workflow::template::TemplateRenderer;
use super::node_executor::NodeExecutor;

/// 验证执行器配置
#[derive(Debug, Clone)]
pub struct ValidateExecutorConfig {
    /// 是否启用 AI 验证
    pub enable_ai_validation: bool,
    /// AI 验证提示模板
    pub ai_validation_prompt: String,
    /// 是否在 AI 验证失败时中止
    pub abort_on_ai_failure: bool,
}

impl Default for ValidateExecutorConfig {
    fn default() -> Self {
        Self {
            enable_ai_validation: false,
            ai_validation_prompt: String::new(),
            abort_on_ai_failure: true,
        }
    }
}

/// 验证执行器
///
/// 混合验证执行器：程序规则验证 + AI 验证。
pub struct ValidateExecutor {
    /// AI Provider（可选）
    provider: Option<Arc<dyn Provider>>,
    /// 配置
    config: ValidateExecutorConfig,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ValidateExecutor {
    /// 创建新的验证执行器（仅程序规则）
    pub fn new() -> Self {
        Self {
            provider: None,
            config: ValidateExecutorConfig::default(),
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 创建带 AI 验证的执行器
    pub fn with_ai(provider: Arc<dyn Provider>, config: ValidateExecutorConfig) -> Self {
        Self {
            provider: Some(provider),
            config,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 执行 AI 验证
    async fn validate_with_ai(
        &self,
        data: &serde_json::Value,
        context: &WorkflowContext,
    ) -> Result<ValidationResult> {
        if let Some(provider) = &self.provider {
            // 构建验证提示
            let prompt = if self.config.ai_validation_prompt.is_empty() {
                format!(
                    "Please validate the following data and return a JSON object with 'passed' (boolean) and 'errors' (array of strings):\n{}",
                    serde_json::to_string_pretty(data)?
                )
            } else {
                self.template_renderer.render(&self.config.ai_validation_prompt, &context.variables)?
            };

            // 构建请求
            let messages = vec![Message {
                role: crate::providers::Role::User,
                content: MessageContent::Text(prompt),
            }];

            let request = ChatRequest {
                messages,
                tools: Vec::new(),
                system: Some("You are a data validator. Return JSON with 'passed' and 'errors' fields.".to_string()),
                think: false,
                max_tokens: 1024,
                server_tools: Vec::new(),
                enable_caching: false,
            };

            // 调用 AI
            let response = provider.chat(request).await?;

            // 解析响应
            for block in &response.content {
                if let ContentBlock::Text { text } = block {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                        let passed = json.get("passed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let errors = json.get("errors")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect())
                            .unwrap_or_default();

                        return Ok(ValidationResult {
                            passed,
                            errors,
                        });
                    }
                }
            }

            // 无法解析 AI 响应
            Ok(ValidationResult::failure("Failed to parse AI validation response".to_string()))
        } else {
            // 没有 AI Provider，直接通过
            Ok(ValidationResult::success())
        }
    }
}

impl Default for ValidateExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NodeExecutor for ValidateExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 从节点参数中提取验证规则
        let rules_json = node.params.get("rules")
            .ok_or_else(|| anyhow::anyhow!("Validate executor requires 'rules' parameter"))?;

        // 解析规则
        let rules: Vec<Rule> = serde_json::from_value(rules_json.clone())
            .with_context(|| "Failed to parse validation rules")?;

        // 创建可变副本用于规则验证
        let mut rule_engine = RuleEngine::new();

        // 执行规则验证
        let mut result = ValidationResult::success();
        for rule in &rules {
            result = result.merge(rule_engine.validate(rule, &context.variables)?);
        }

        // 如果规则验证通过且有 AI Provider，执行 AI 验证
        if result.passed && self.config.enable_ai_validation && self.provider.is_some() {
            // 将 HashMap 转换为 serde_json::Map
            let context_vars: serde_json::Map<String, serde_json::Value> = context
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let data_to_validate = node.params.get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Object(context_vars));

            let ai_result = self.validate_with_ai(&data_to_validate, context).await?;
            result = result.merge(ai_result);
        }

        // 构建输出
        let output = serde_json::json!({
            "passed": result.passed,
            "errors": result.errors,
            "node_id": node.id,
        });

        // 如果验证失败且配置为中止，返回错误
        if !result.passed && self.config.abort_on_ai_failure {
            return Err(anyhow::anyhow!("Validation failed: {}", result.errors.join("; ")));
        }

        Ok(output)
    }

    fn name(&self) -> &str {
        "validate_executor"
    }
}