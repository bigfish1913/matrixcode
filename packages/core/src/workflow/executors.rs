//! Workflow Executors
//!
//! 工作流节点执行器，提供不同类型节点的执行实现。
//!
//! # 执行器类型
//!
//! - `AiExecutor`: AI 模型调用执行器（调用 Provider）
//! - `ToolExecutor`: 工具调用执行器（调用现有 Tools）
//! - `ConditionExecutor`: 条件判断执行器（调用 rule_engine）
//! - `ValidateExecutor`: 混合验证执行器（程序规则 + AI 验证）

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::providers::{ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider};
use crate::tools::{Tool, ToolDefinition};
use super::context::WorkflowContext;
use super::def::NodeDef;
use super::rule_engine::{Rule, RuleEngine, ValidationResult, evaluate_expression};
use super::template::TemplateRenderer;

/// NodeExecutor trait - 节点执行器接口
///
/// 所有节点执行器必须实现此接口，支持异步执行和错误处理。
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    /// 执行节点
    ///
    /// # 参数
    ///
    /// - `node`: 节点定义
    /// - `context`: 工作流上下文
    ///
    /// # 返回
    ///
    /// 执行结果，包含输出数据
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value>;

    /// 执行器名称（用于日志和调试）
    fn name(&self) -> &str;
}

/// AI 执行器配置
#[derive(Debug, Clone)]
pub struct AiExecutorConfig {
    /// 系统提示模板
    pub system_prompt: Option<String>,
    /// 最大输出 token 数
    pub max_tokens: u32,
    /// 是否启用思考模式
    pub enable_thinking: bool,
    /// 是否启用流式输出
    pub enable_streaming: bool,
}

impl Default for AiExecutorConfig {
    fn default() -> Self {
        Self {
            system_prompt: None,
            max_tokens: 4096,
            enable_thinking: false,
            enable_streaming: false,
        }
    }
}

/// AI 执行器
///
/// 调用 AI Provider 执行任务节点。
pub struct AiExecutor {
    /// Provider 实例
    provider: Arc<dyn Provider>,
    /// 配置
    config: AiExecutorConfig,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl AiExecutor {
    /// 创建新的 AI 执行器
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            config: AiExecutorConfig::default(),
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 使用配置创建 AI 执行器
    pub fn with_config(provider: Arc<dyn Provider>, config: AiExecutorConfig) -> Self {
        Self {
            provider,
            config,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 从响应中提取文本内容
    fn extract_text_content(response: &ChatResponse) -> Result<String> {
        let mut text_parts = Vec::new();
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text.clone());
                }
                ContentBlock::Thinking { thinking, .. } => {
                    text_parts.push(format!("[Thinking]\n{}", thinking));
                }
                _ => {}
            }
        }
        Ok(text_parts.join("\n"))
    }

    /// 从响应中提取结构化输出
    fn extract_structured_output(response: &ChatResponse) -> Result<serde_json::Value> {
        // 尝试从文本中解析 JSON
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                // 尝试解析为 JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    return Ok(json);
                }
            }
        }

        // 如果没有找到 JSON，返回文本内容
        let text = Self::extract_text_content(response)?;
        let stop_reason_str = match response.stop_reason {
            crate::providers::StopReason::EndTurn => "end_turn",
            crate::providers::StopReason::ToolUse => "tool_use",
            crate::providers::StopReason::MaxTokens => "max_tokens",
        };
        Ok(serde_json::json!({
            "text": text,
            "stop_reason": stop_reason_str,
            "usage": {
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens,
            }
        }))
    }
}

#[async_trait]
impl NodeExecutor for AiExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取任务名称
        let task_name = node.task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("AI executor requires a task name"))?;

        // 构建用户消息
        let mut prompt_parts = Vec::new();

        // 添加任务名称
        prompt_parts.push(format!("Task: {}", task_name));

        // 添加任务描述
        if let Some(desc) = &node.description {
            prompt_parts.push(format!("Description: {}", desc));
        }

        // 渲染并添加参数
        for (key, value) in &node.params {
            let rendered_value = if let serde_json::Value::String(s) = value {
                self.template_renderer.render(s, &context.variables)?
            } else {
                value.to_string()
            };
            prompt_parts.push(format!("{}: {}", key, rendered_value));
        }

        // 添加上下文信息
        if !context.variables.is_empty() {
            prompt_parts.push("\nContext:".to_string());
            for (key, value) in &context.variables {
                prompt_parts.push(format!("  {}: {}", key, value));
            }
        }

        let user_message = prompt_parts.join("\n");

        // 构建聊天请求
        let messages = vec![Message {
            role: crate::providers::Role::User,
            content: MessageContent::Text(user_message),
        }];

        let request = ChatRequest {
            messages,
            tools: Vec::new(),
            system: self.config.system_prompt.clone(),
            think: self.config.enable_thinking,
            max_tokens: self.config.max_tokens,
            server_tools: Vec::new(),
            enable_caching: false,
        };

        // 调用 Provider
        let response = self.provider.chat(request)
            .await
            .with_context(|| format!("AI executor failed for task '{}'", task_name))?;

        // 提取输出
        let output = Self::extract_structured_output(&response)?;

        // 更新上下文
        let output_ref = &output;
        if let serde_json::Value::Object(map) = output_ref {
            for (key, value) in map {
                context.set_variable(key.clone(), value.clone());
            }
        }

        Ok(output)
    }

    fn name(&self) -> &str {
        "ai_executor"
    }
}

/// 工具执行器配置
#[derive(Debug, Clone)]
pub struct ToolExecutorConfig {
    /// 是否记录工具调用结果
    pub log_results: bool,
    /// 是否允许工具调用失败
    pub allow_failure: bool,
}

impl Default for ToolExecutorConfig {
    fn default() -> Self {
        Self {
            log_results: true,
            allow_failure: false,
        }
    }
}

/// 工具执行器
///
/// 调用现有的 Tools 系统执行任务节点。
pub struct ToolExecutor {
    /// 工具集合
    tools: HashMap<String, Arc<dyn Tool>>,
    /// 配置
    config: ToolExecutorConfig,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ToolExecutor {
    /// 创建新的工具执行器
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        let mut tool_map = HashMap::new();
        for tool in tools {
            let def = tool.definition();
            tool_map.insert(def.name, Arc::from(tool));
        }
        Self {
            tools: tool_map,
            config: ToolExecutorConfig::default(),
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 使用配置创建工具执行器
    pub fn with_config(tools: Vec<Box<dyn Tool>>, config: ToolExecutorConfig) -> Self {
        let mut tool_map = HashMap::new();
        for tool in tools {
            let def = tool.definition();
            tool_map.insert(def.name, Arc::from(tool));
        }
        Self {
            tools: tool_map,
            config,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 注册单个工具
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        let def = tool.definition();
        self.tools.insert(def.name, Arc::from(tool));
    }

    /// 渲染参数
    fn render_params(
        &self,
        params: &HashMap<String, serde_json::Value>,
        context: &WorkflowContext,
    ) -> Result<serde_json::Value> {
        let mut rendered = HashMap::new();
        for (key, value) in params {
            let rendered_value = if let serde_json::Value::String(s) = value {
                let rendered_str = self.template_renderer.render(s, &context.variables)?;
                serde_json::Value::String(rendered_str)
            } else {
                value.clone()
            };
            rendered.insert(key.clone(), rendered_value);
        }
        Ok(serde_json::Value::Object(rendered.into_iter().collect()))
    }

    /// 检查工具是否存在
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 获取工具定义
    pub fn get_tool_definition(&self, name: &str) -> Option<ToolDefinition> {
        self.tools.get(name).map(|t| t.definition())
    }

    /// 获取所有工具定义
    pub fn get_all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|(_, t)| t.definition()).collect()
    }
}

#[async_trait]
impl NodeExecutor for ToolExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取工具名称
        let tool_name = node.task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool executor requires a task name"))?;

        // 查找工具
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", tool_name))?;

        // 渲染参数
        let params = self.render_params(&node.params, context)?;

        // 执行工具
        let result = tool.execute(params.clone())
            .await
            .with_context(|| format!("Tool '{}' execution failed", tool_name));

        // 处理结果
        match result {
            Ok(output_str) => {
                // 尝试解析为 JSON
                let output = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
                    json
                } else {
                    serde_json::json!({
                        "result": output_str,
                        "tool": tool_name,
                    })
                };

                // 更新上下文
                if let serde_json::Value::Object(map) = &output {
                    for (key, value) in map {
                        context.set_variable(key.clone(), value.clone());
                    }
                }

                Ok(output)
            }
            Err(e) => {
                if self.config.allow_failure {
                    Ok(serde_json::json!({
                        "error": e.to_string(),
                        "tool": tool_name,
                        "success": false,
                    }))
                } else {
                    Err(e)
                }
            }
        }
    }

    fn name(&self) -> &str {
        "tool_executor"
    }
}

/// 条件执行器
///
/// 使用 rule_engine 进行条件判断和分支选择。
pub struct ConditionExecutor {
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ConditionExecutor {
    /// 创建新的条件执行器
    pub fn new() -> Self {
        Self {
            template_renderer: TemplateRenderer::new(),
        }
    }
}

impl Default for ConditionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NodeExecutor for ConditionExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 条件节点必须有分支定义
        let branches = node.branches.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Condition node '{}' has no branches", node.id))?;

        // 遍历所有分支，找到匹配的
        for branch in branches {
            // 渲染条件表达式
            let rendered_condition = self.template_renderer
                .render(&branch.condition, &context.variables)?;

            // 评估条件
            let passed = evaluate_expression(&rendered_condition, &context.variables)?;

            if passed {
                // 找到匹配分支，返回目标节点
                return Ok(serde_json::json!({
                    "matched_branch": branch.name,
                    "target": branch.target,
                    "condition": branch.condition,
                }));
            }
        }

        // 没有匹配的分支
        Ok(serde_json::json!({
            "matched": false,
            "branches_checked": branches.len(),
        }))
    }

    fn name(&self) -> &str {
        "condition_executor"
    }
}

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

/// 组合执行器
///
/// 可以组合多个执行器，按顺序或条件执行。
pub struct CompositeExecutor {
    /// 子执行器列表
    executors: Vec<Arc<dyn NodeExecutor>>,
    /// 执行模式
    mode: CompositeMode,
}

/// 组合执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeMode {
    /// 按顺序执行所有
    Sequential,
    /// 执行第一个成功的
    FirstSuccess,
    /// 并行执行（需要 tokio）
    Parallel,
}

impl CompositeExecutor {
    /// 创建新的组合执行器
    pub fn new(executors: Vec<Arc<dyn NodeExecutor>>, mode: CompositeMode) -> Self {
        Self { executors, mode }
    }

    /// 添加执行器
    pub fn add_executor(&mut self, executor: Arc<dyn NodeExecutor>) {
        self.executors.push(executor);
    }
}

#[async_trait]
impl NodeExecutor for CompositeExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        match self.mode {
            CompositeMode::Sequential => {
                let mut outputs = Vec::new();
                for executor in &self.executors {
                    let output = executor.execute(node, context).await?;
                    outputs.push(output);
                }
                Ok(serde_json::Value::Array(outputs))
            }
            CompositeMode::FirstSuccess => {
                for executor in &self.executors {
                    if let Ok(output) = executor.execute(node, context).await {
                        return Ok(output);
                    }
                }
                Err(anyhow::anyhow!("All executors failed"))
            }
            CompositeMode::Parallel => {
                // 并行执行需要更复杂的实现
                // 这里简化为顺序执行
                let mut outputs = Vec::new();
                for executor in &self.executors {
                    let output = executor.execute(node, context).await?;
                    outputs.push(output);
                }
                Ok(serde_json::Value::Array(outputs))
            }
        }
    }

    fn name(&self) -> &str {
        "composite_executor"
    }
}

/// 执行器工厂
///
/// 用于创建和管理各种执行器实例。
pub struct ExecutorFactory {
    /// Provider 实例（用于 AI 执行器）
    provider: Option<Arc<dyn Provider>>,
    /// 工具名称列表（用于延迟创建工具执行器）
    tool_names: Vec<String>,
}

impl ExecutorFactory {
    /// 创建新的执行器工厂
    pub fn new() -> Self {
        Self {
            provider: None,
            tool_names: Vec::new(),
        }
    }

    /// 设置 Provider
    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置工具名称列表
    pub fn with_tool_names(mut self, names: Vec<String>) -> Self {
        self.tool_names = names;
        self
    }

    /// 创建 AI 执行器
    pub fn create_ai_executor(&self) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI executor"))?;
        Ok(Arc::new(AiExecutor::new(provider)))
    }

    /// 创建带配置的 AI 执行器
    pub fn create_ai_executor_with_config(&self, config: AiExecutorConfig) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI executor"))?;
        Ok(Arc::new(AiExecutor::with_config(provider, config)))
    }

    /// 创建工具执行器（使用默认工具集）
    pub fn create_tool_executor(&self) -> Arc<dyn NodeExecutor> {
        Arc::new(ToolExecutor::new(crate::tools::all_tools()))
    }

    /// 创建条件执行器
    pub fn create_condition_executor(&self) -> Arc<dyn NodeExecutor> {
        Arc::new(ConditionExecutor::new())
    }

    /// 创建验证执行器
    pub fn create_validate_executor(&self) -> Arc<dyn NodeExecutor> {
        Arc::new(ValidateExecutor::new())
    }

    /// 创建带 AI 的验证执行器
    pub fn create_validate_executor_with_ai(&self, config: ValidateExecutorConfig) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI validation"))?;
        Ok(Arc::new(ValidateExecutor::with_ai(provider, config)))
    }

    /// 根据任务类型创建执行器
    pub fn create_executor_for_task(&self, task_type: &str) -> Result<Arc<dyn NodeExecutor>> {
        match task_type {
            "ai" | "claude" | "gpt" => self.create_ai_executor(),
            "tool" | "bash" | "read" | "write" | "edit" => Ok(self.create_tool_executor()),
            "condition" | "branch" => Ok(self.create_condition_executor()),
            "validate" | "check" => Ok(self.create_validate_executor()),
            _ => Err(anyhow::anyhow!("Unknown task type: {}", task_type)),
        }
    }
}

impl Default for ExecutorFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{StopReason, Usage};
    use crate::tools::bash::BashTool;

    // Mock Provider for testing
    struct MockProvider;

    #[async_trait]
    impl Provider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                content: vec![ContentBlock::Text { text: "{\"result\": \"mock response\"}".to_string() }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        fn clone_box(&self) -> Box<dyn Provider> {
            Box::new(MockProvider)
        }
    }

    #[tokio::test]
    async fn test_ai_executor() {
        let provider = Arc::new(MockProvider);
        let executor = AiExecutor::new(provider);

        let node = NodeDef {
            id: "test".to_string(),
            node_type: super::super::def::NodeType::Task,
            name: "Test Node".to_string(),
            description: Some("Test AI execution".to_string()),
            task: Some("generate_text".to_string()),
            params: HashMap::new(),
            on_failure: super::super::def::FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };

        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.is_object());
    }

    #[test]
    fn test_tool_executor_creation() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(BashTool),
        ];
        let executor = ToolExecutor::new(tools);

        assert!(executor.has_tool("bash"));
        assert!(!executor.has_tool("nonexistent"));
    }

    #[test]
    fn test_condition_executor() {
        let executor = ConditionExecutor::new();
        assert_eq!(executor.name(), "condition_executor");
    }

    #[test]
    fn test_validate_executor() {
        let executor = ValidateExecutor::new();
        assert_eq!(executor.name(), "validate_executor");
    }

    #[test]
    fn test_executor_factory() {
        let factory = ExecutorFactory::new();
        let condition_executor = factory.create_condition_executor();
        assert_eq!(condition_executor.name(), "condition_executor");
    }

    #[tokio::test]
    async fn test_composite_executor_sequential() {
        let executors: Vec<Arc<dyn NodeExecutor>> = vec![
            Arc::new(ConditionExecutor::new()),
            Arc::new(ValidateExecutor::new()),
        ];

        let composite = CompositeExecutor::new(executors, CompositeMode::Sequential);

        let node = NodeDef {
            id: "composite-test".to_string(),
            node_type: super::super::def::NodeType::Task,
            name: "Composite Test".to_string(),
            description: None,
            task: Some("validate".to_string()),
            params: HashMap::from([
                ("rules".to_string(), serde_json::json!([])),
            ]),
            on_failure: super::super::def::FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };

        let mut context = WorkflowContext::new("test".to_string(), HashMap::new());
        let result = composite.execute(&node, &mut context).await;
        assert!(result.is_ok());
    }
}