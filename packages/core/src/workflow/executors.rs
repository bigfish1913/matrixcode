//! Workflow Executors
//!
//! 工作流节点执行器，提供不同类型节点的执行实现。
//!
//! # 执行器类型
//!
//! - `AiExecutor`: AI 模型调用执行器（调用 Provider）
//! - `ToolExecutor`: 工具调用执行器（调用现有 Tools）
//! - `ProxyExecutor`: 代理工具执行器（调用 ProxyToolExecutor）
//! - `ConditionExecutor`: 条件判断执行器（调用 rule_engine）
//! - `ValidateExecutor`: 混合验证执行器（程序规则 + AI 验证）

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::providers::{ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider};
use crate::tools::{Tool, ToolDefinition};
use crate::tools::toolproxy::{ProxyToolExecutor, ProxyToolDef};
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

/// 代理工具执行器
///
/// 包装 ProxyToolExecutor 为 NodeExecutor，让 workflow 可以调用代理工具。
pub struct ProxyExecutor {
    /// 代理工具执行器
    executor: Arc<dyn ProxyToolExecutor>,
    /// 工具定义列表
    tool_defs: Vec<ProxyToolDef>,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ProxyExecutor {
    /// 创建新的代理执行器
    pub fn new(executor: Arc<dyn ProxyToolExecutor>, tool_defs: Vec<ProxyToolDef>) -> Self {
        Self {
            executor,
            tool_defs,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 检查是否支持该工具
    pub fn has_tool(&self, name: &str) -> bool {
        self.tool_defs.iter().any(|t| t.definition.name == name)
    }

    /// 获取工具超时时间
    pub fn get_timeout(&self, name: &str) -> u64 {
        self.tool_defs
            .iter()
            .find(|t| t.definition.name == name)
            .map(|t| t.timeout_ms)
            .unwrap_or(30000)
    }
}

#[async_trait]
impl NodeExecutor for ProxyExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取工具名称
        let tool_name = node.task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Proxy executor requires a task name"))?;

        // 检查工具是否存在
        if !self.has_tool(tool_name) {
            return Err(anyhow::anyhow!("Proxy tool '{}' not found", tool_name));
        }

        // 渲染参数
        let params = self.template_renderer.render_params(&node.params, &context.variables)?;

        // 执行代理工具
        let result = self.executor.exec(tool_name, params.clone()).await
            .with_context(|| format!("Proxy tool '{}' execution failed", tool_name))?;

        // 解析结果
        let output = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result) {
            json
        } else {
            serde_json::json!({
                "result": result,
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

    fn name(&self) -> &str {
        "proxy_executor"
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
        let tools = if let Some(provider) = &self.provider {
            crate::tools::all_tools_with_provider(
                std::sync::Arc::new(Vec::new()),
                provider.clone()
            )
        } else {
            crate::tools::all_tools()
        };
        Arc::new(ToolExecutor::new(tools))
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
    use crate::workflow::context::WorkflowContext;
    use crate::workflow::def::{NodeType, FailureStrategy, BranchDef, NodeDef};
    use crate::workflow::rule_engine::Rule;
    use crate::tools::bash::BashTool;
    use std::collections::HashMap;
    use serde_json::json;

    // ============================================================================
    // Mock Provider for AI Executor Testing
    // ============================================================================

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

    /// Mock Provider that returns a configurable response
    struct ConfigurableMockProvider {
        response_text: String,
        response_json: Option<serde_json::Value>,
    }

    impl ConfigurableMockProvider {
        fn with_json(json: serde_json::Value) -> Self {
            Self {
                response_text: serde_json::to_string(&json).unwrap(),
                response_json: Some(json),
            }
        }
    }

    #[async_trait]
    impl Provider for ConfigurableMockProvider {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                content: vec![ContentBlock::Text { text: self.response_text.clone() }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        fn clone_box(&self) -> Box<dyn Provider> {
            Box::new(ConfigurableMockProvider {
                response_text: self.response_text.clone(),
                response_json: self.response_json.clone(),
            })
        }
    }

    // ============================================================================
    // AiExecutor Tests
    // ============================================================================

    #[tokio::test]
    async fn test_ai_executor_basic() {
        let provider = Arc::new(MockProvider);
        let executor = AiExecutor::new(provider);

        let node = create_task_node("test-ai", "generate_text", HashMap::new());
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.is_object());
        assert_eq!(output.get("result").unwrap(), &json!("mock response"));
    }

    #[tokio::test]
    async fn test_ai_executor_with_params() {
        let provider = Arc::new(MockProvider);
        let executor = AiExecutor::new(provider);

        let params = HashMap::from([
            ("topic".to_string(), json!("testing")),
            ("count".to_string(), json!(5)),
        ]);
        let node = create_task_node("test-ai-params", "analyze", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ai_executor_with_template_params() {
        let provider = Arc::new(ConfigurableMockProvider::with_json(json!({
            "analysis": "complete",
            "score": 95
        })));
        let executor = AiExecutor::new(provider);

        let params = HashMap::from([
            ("input".to_string(), json!("{{user_input}}")),
        ]);
        let node = create_task_node("test-template", "process", params);

        let inputs = HashMap::from([("user_input".to_string(), json!("hello world"))]);
        let mut context = WorkflowContext::new("test-workflow".to_string(), inputs);

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_ok());

        // Check that context was updated with output
        assert!(context.get_variable("analysis").is_some());
        assert_eq!(context.get_variable("score").unwrap(), &json!(95));
    }

    #[tokio::test]
    async fn test_ai_executor_with_config() {
        let provider = Arc::new(MockProvider);
        let config = AiExecutorConfig {
            system_prompt: Some("You are a test assistant".to_string()),
            max_tokens: 2048,
            enable_thinking: true,
            enable_streaming: false,
        };
        let executor = AiExecutor::with_config(provider, config);

        let node = create_task_node("test-config", "test_task", HashMap::new());
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_ok());
        assert_eq!(executor.name(), "ai_executor");
    }

    #[tokio::test]
    async fn test_ai_executor_without_task_name() {
        let provider = Arc::new(MockProvider);
        let executor = AiExecutor::new(provider);

        // Node without task name should fail
        let node = NodeDef {
            id: "no-task".to_string(),
            node_type: NodeType::Task,
            name: "No Task".to_string(),
            description: None,
            task: None, // No task name
            params: HashMap::new(),
            on_failure: FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a task name"));
    }

    #[test]
    fn test_ai_executor_extract_text_content() {
        let response = ChatResponse {
            content: vec![
                ContentBlock::Text { text: "Hello".to_string() },
                ContentBlock::Thinking { thinking: "Let me think...".to_string(), signature: None },
                ContentBlock::Text { text: "World".to_string() },
            ],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        };

        let text = AiExecutor::extract_text_content(&response).unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("[Thinking]"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_ai_executor_extract_structured_output() {
        let response = ChatResponse {
            content: vec![ContentBlock::Text { text: "{\"key\": \"value\"}".to_string() }],
            stop_reason: StopReason::EndTurn,
            usage: Usage { input_tokens: 100, output_tokens: 50, ..Default::default() },
        };

        let output = AiExecutor::extract_structured_output(&response).unwrap();
        assert_eq!(output.get("key").unwrap(), &json!("value"));
    }

    #[test]
    fn test_ai_executor_extract_structured_output_non_json() {
        let response = ChatResponse {
            content: vec![ContentBlock::Text { text: "plain text response".to_string() }],
            stop_reason: StopReason::MaxTokens,
            usage: Usage { input_tokens: 100, output_tokens: 50, ..Default::default() },
        };

        let output = AiExecutor::extract_structured_output(&response).unwrap();
        assert_eq!(output.get("text").unwrap(), &json!("plain text response"));
        assert_eq!(output.get("stop_reason").unwrap(), &json!("max_tokens"));
    }

    #[test]
    fn test_ai_executor_config_default() {
        let config = AiExecutorConfig::default();
        assert!(config.system_prompt.is_none());
        assert_eq!(config.max_tokens, 4096);
        assert!(!config.enable_thinking);
        assert!(!config.enable_streaming);
    }

    // ============================================================================
    // ToolExecutor Tests
    // ============================================================================

    #[test]
    fn test_tool_executor_creation() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(BashTool)];
        let executor = ToolExecutor::new(tools);

        assert!(executor.has_tool("bash"));
        assert!(!executor.has_tool("nonexistent"));
        assert_eq!(executor.name(), "tool_executor");
    }

    #[test]
    fn test_tool_executor_register_tool() {
        let mut executor = ToolExecutor::new(vec![]);
        assert!(!executor.has_tool("bash"));

        executor.register_tool(Box::new(BashTool));
        assert!(executor.has_tool("bash"));
    }

    #[test]
    fn test_tool_executor_get_definitions() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(BashTool)];
        let executor = ToolExecutor::new(tools);

        let bash_def = executor.get_tool_definition("bash");
        assert!(bash_def.is_some());
        assert_eq!(bash_def.unwrap().name, "bash");

        let all_defs = executor.get_all_tool_definitions();
        assert_eq!(all_defs.len(), 1);
    }

    #[test]
    fn test_tool_executor_config_default() {
        let config = ToolExecutorConfig::default();
        assert!(config.log_results);
        assert!(!config.allow_failure);
    }

    #[tokio::test]
    async fn test_tool_executor_missing_tool() {
        let executor = ToolExecutor::new(vec![]);

        let node = create_task_node("test-tool", "nonexistent_tool", HashMap::new());
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_tool_executor_without_task_name() {
        let executor = ToolExecutor::new(vec![Box::new(BashTool)]);

        let node = NodeDef {
            id: "no-task".to_string(),
            node_type: NodeType::Task,
            name: "No Task".to_string(),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a task name"));
    }

    #[test]
    fn test_tool_executor_render_params() {
        let executor = ToolExecutor::new(vec![]);

        let params = HashMap::from([
            ("path".to_string(), json!("{{base_path}}/file.txt")),
            ("count".to_string(), json!(10)),
        ]);

        let mut context = WorkflowContext::new("test".to_string(), HashMap::new());
        context.set_variable("base_path".to_string(), json!("/home/user"));

        let rendered = executor.render_params(&params, &context).unwrap();
        assert_eq!(rendered.get("path").unwrap(), &json!("/home/user/file.txt"));
        assert_eq!(rendered.get("count").unwrap(), &json!(10));
    }

    // ============================================================================
    // ConditionExecutor Tests
    // ============================================================================

    #[test]
    fn test_condition_executor_creation() {
        let executor = ConditionExecutor::new();
        assert_eq!(executor.name(), "condition_executor");
    }

    #[test]
    fn test_condition_executor_default() {
        let executor = ConditionExecutor::default();
        assert_eq!(executor.name(), "condition_executor");
    }

    #[tokio::test]
    async fn test_condition_executor_with_branches() {
        let executor = ConditionExecutor::new();

        let branches = vec![
            BranchDef {
                name: "high_score".to_string(),
                condition: "score > 80".to_string(),
                target: "success_node".to_string(),
            },
            BranchDef {
                name: "low_score".to_string(),
                condition: "score < 50".to_string(),
                target: "failure_node".to_string(),
            },
        ];

        let node = create_condition_node("cond-test", branches);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("score".to_string(), json!(90));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("matched_branch").unwrap(), &json!("high_score"));
        assert_eq!(result.get("target").unwrap(), &json!("success_node"));
    }

    #[tokio::test]
    async fn test_condition_executor_no_match() {
        let executor = ConditionExecutor::new();

        let branches = vec![
            BranchDef {
                name: "high".to_string(),
                condition: "score > 100".to_string(),
                target: "high_node".to_string(),
            },
        ];

        let node = create_condition_node("cond-no-match", branches);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("score".to_string(), json!(50));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("matched").unwrap(), &json!(false));
        assert_eq!(result.get("branches_checked").unwrap(), &json!(1));
    }

    #[tokio::test]
    async fn test_condition_executor_without_branches() {
        let executor = ConditionExecutor::new();

        // Node without branches should fail
        let node = NodeDef {
            id: "no-branches".to_string(),
            node_type: NodeType::Condition,
            name: "No Branches".to_string(),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("has no branches"));
    }

    #[tokio::test]
    async fn test_condition_executor_with_template_condition() {
        let executor = ConditionExecutor::new();

        // Template condition: {{count}} > 5
        // After rendering with count=10, becomes: 10 > 5
        let branches = vec![
            BranchDef {
                name: "match".to_string(),
                condition: "{{count}} > 5".to_string(),
                target: "next_node".to_string(),
            },
        ];

        let node = create_condition_node("template-cond", branches);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("count".to_string(), json!(10));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("matched_branch").unwrap(), &json!("match"));
    }

    #[tokio::test]
    async fn test_condition_executor_complex_conditions() {
        let executor = ConditionExecutor::new();

        let branches = vec![
            BranchDef {
                name: "complex_true".to_string(),
                condition: "count > 5 && enabled == true".to_string(),
                target: "complex_node".to_string(),
            },
        ];

        let node = create_condition_node("complex-cond", branches);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("count".to_string(), json!(10));
        context.set_variable("enabled".to_string(), json!(true));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("matched_branch").unwrap(), &json!("complex_true"));
    }

    // ============================================================================
    // ValidateExecutor Tests
    // ============================================================================

    #[test]
    fn test_validate_executor_creation() {
        let executor = ValidateExecutor::new();
        assert_eq!(executor.name(), "validate_executor");
    }

    #[test]
    fn test_validate_executor_default() {
        let executor = ValidateExecutor::default();
        assert_eq!(executor.name(), "validate_executor");
    }

    #[test]
    fn test_validate_executor_config_default() {
        let config = ValidateExecutorConfig::default();
        assert!(!config.enable_ai_validation);
        assert!(config.ai_validation_prompt.is_empty());
        assert!(config.abort_on_ai_failure);
    }

    #[tokio::test]
    async fn test_validate_executor_with_rules() {
        let executor = ValidateExecutor::new();

        let rules = vec![
            Rule::Equals {
                field: "status".to_string(),
                value: json!("ready"),
            },
        ];

        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("validate-test", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("status".to_string(), json!("ready"));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("passed").unwrap(), &json!(true));
        assert!(result.get("errors").unwrap().as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_validate_executor_fails_validation() {
        let executor = ValidateExecutor::new();

        let rules = vec![
            Rule::Equals {
                field: "status".to_string(),
                value: json!("ready"),
            },
        ];

        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("validate-fail", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("status".to_string(), json!("pending"));

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Validation failed"));
    }

    #[tokio::test]
    async fn test_validate_executor_without_rules() {
        let executor = ValidateExecutor::new();

        let node = create_task_node("no-rules", "validate", HashMap::new());
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'rules'"));
    }

    #[tokio::test]
    async fn test_validate_executor_multiple_rules() {
        let executor = ValidateExecutor::new();

        let rules = vec![
            Rule::Equals {
                field: "status".to_string(),
                value: json!("ready"),
            },
            Rule::GreaterThan {
                field: "count".to_string(),
                value: 10.0,
            },
        ];

        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("multi-rules", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        context.set_variable("status".to_string(), json!("ready"));
        context.set_variable("count".to_string(), json!(15));

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("passed").unwrap(), &json!(true));
    }

    #[tokio::test]
    async fn test_validate_executor_with_ai_validation() {
        let provider = Arc::new(ConfigurableMockProvider::with_json(json!({
            "passed": true,
            "errors": []
        })));
        let config = ValidateExecutorConfig {
            enable_ai_validation: true,
            ai_validation_prompt: "Validate this data".to_string(),
            abort_on_ai_failure: true,
        };
        let executor = ValidateExecutor::with_ai(provider, config);

        let rules: Vec<Rule> = vec![];
        let params = HashMap::from([
            ("rules".to_string(), serde_json::to_value(&rules).unwrap()),
            ("data".to_string(), json!({"key": "value"})),
        ]);
        let node = create_task_node("ai-validate", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("passed").unwrap(), &json!(true));
    }

    #[tokio::test]
    async fn test_validate_executor_ai_validation_fails() {
        let provider = Arc::new(ConfigurableMockProvider::with_json(json!({
            "passed": false,
            "errors": ["Invalid format"]
        })));
        let config = ValidateExecutorConfig {
            enable_ai_validation: true,
            ai_validation_prompt: "Validate this data".to_string(),
            abort_on_ai_failure: true,
        };
        let executor = ValidateExecutor::with_ai(provider, config);

        let rules: Vec<Rule> = vec![];
        let params = HashMap::from([
            ("rules".to_string(), serde_json::to_value(&rules).unwrap()),
            ("data".to_string(), json!({"key": "value"})),
        ]);
        let node = create_task_node("ai-fail", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_executor_ai_validation_no_abort() {
        let provider = Arc::new(ConfigurableMockProvider::with_json(json!({
            "passed": false,
            "errors": ["Invalid format"]
        })));
        let config = ValidateExecutorConfig {
            enable_ai_validation: true,
            ai_validation_prompt: "Validate this data".to_string(),
            abort_on_ai_failure: false, // Don't abort on failure
        };
        let executor = ValidateExecutor::with_ai(provider, config);

        let rules: Vec<Rule> = vec![];
        let params = HashMap::from([
            ("rules".to_string(), serde_json::to_value(&rules).unwrap()),
            ("data".to_string(), json!({"key": "value"})),
        ]);
        let node = create_task_node("ai-no-abort", "validate", params);
        let mut context = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        let result = executor.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("passed").unwrap(), &json!(false));
        assert!(!result.get("errors").unwrap().as_array().unwrap().is_empty());
    }

    // ============================================================================
    // CompositeExecutor Tests
    // ============================================================================

    #[tokio::test]
    async fn test_composite_executor_sequential_mode() {
        let rules: Vec<Rule> = vec![];
        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("sequential-test", "validate", params);

        let executors: Vec<Arc<dyn NodeExecutor>> = vec![
            Arc::new(ValidateExecutor::new()),
            Arc::new(ValidateExecutor::new()),
        ];

        let composite = CompositeExecutor::new(executors, CompositeMode::Sequential);
        let mut context = WorkflowContext::new("test".to_string(), HashMap::new());

        let result = composite.execute(&node, &mut context).await.unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 2);
        assert_eq!(composite.name(), "composite_executor");
    }

    #[tokio::test]
    async fn test_composite_executor_first_success_mode() {
        let rules = vec![Rule::Equals {
            field: "status".to_string(),
            value: json!("ready"),
        }];
        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("first-success", "validate", params);

        let executors: Vec<Arc<dyn NodeExecutor>> = vec![
            Arc::new(ValidateExecutor::new()),
            Arc::new(ValidateExecutor::new()),
        ];

        let composite = CompositeExecutor::new(executors, CompositeMode::FirstSuccess);
        let mut context = WorkflowContext::new("test".to_string(), HashMap::new());
        context.set_variable("status".to_string(), json!("ready"));

        let result = composite.execute(&node, &mut context).await.unwrap();
        assert_eq!(result.get("passed").unwrap(), &json!(true));
    }

    #[tokio::test]
    async fn test_composite_executor_first_success_all_fail() {
        let rules = vec![Rule::Equals {
            field: "status".to_string(),
            value: json!("ready"),
        }];
        let params = HashMap::from([("rules".to_string(), serde_json::to_value(&rules).unwrap())]);
        let node = create_task_node("all-fail", "validate", params);

        let executors: Vec<Arc<dyn NodeExecutor>> = vec![
            Arc::new(ValidateExecutor::new()),
        ];

        let composite = CompositeExecutor::new(executors, CompositeMode::FirstSuccess);
        let mut context = WorkflowContext::new("test".to_string(), HashMap::new());
        context.set_variable("status".to_string(), json!("not_ready"));

        let result = composite.execute(&node, &mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("All executors failed"));
    }

    #[test]
    fn test_composite_executor_add_executor() {
        let mut composite = CompositeExecutor::new(vec![], CompositeMode::Sequential);
        assert_eq!(composite.executors.len(), 0);

        composite.add_executor(Arc::new(ValidateExecutor::new()));
        assert_eq!(composite.executors.len(), 1);
    }

    // ============================================================================
    // ExecutorFactory Tests
    // ============================================================================

    #[test]
    fn test_executor_factory_creation() {
        let factory = ExecutorFactory::new();
        assert!(factory.provider.is_none());
        assert!(factory.tool_names.is_empty());
    }

    #[test]
    fn test_executor_factory_default() {
        let factory = ExecutorFactory::default();
        assert!(factory.provider.is_none());
    }

    #[test]
    fn test_executor_factory_with_provider() {
        let provider = Arc::new(MockProvider);
        let factory = ExecutorFactory::new().with_provider(provider);
        assert!(factory.provider.is_some());
    }

    #[test]
    fn test_executor_factory_with_tool_names() {
        let factory = ExecutorFactory::new().with_tool_names(vec!["bash".to_string(), "read".to_string()]);
        assert_eq!(factory.tool_names.len(), 2);
    }

    #[test]
    fn test_executor_factory_create_condition_executor() {
        let factory = ExecutorFactory::new();
        let executor = factory.create_condition_executor();
        assert_eq!(executor.name(), "condition_executor");
    }

    #[test]
    fn test_executor_factory_create_validate_executor() {
        let factory = ExecutorFactory::new();
        let executor = factory.create_validate_executor();
        assert_eq!(executor.name(), "validate_executor");
    }

    #[test]
    fn test_executor_factory_create_tool_executor() {
        let factory = ExecutorFactory::new();
        let executor = factory.create_tool_executor();
        assert_eq!(executor.name(), "tool_executor");
    }

    #[test]
    fn test_executor_factory_create_ai_executor_without_provider() {
        let factory = ExecutorFactory::new();
        let result = factory.create_ai_executor();
        match result {
            Err(e) => assert!(e.to_string().contains("Provider not configured")),
            Ok(_) => panic!("Should fail without provider"),
        }
    }

    #[test]
    fn test_executor_factory_create_ai_executor_with_provider() {
        let provider = Arc::new(MockProvider);
        let factory = ExecutorFactory::new().with_provider(provider);
        let result = factory.create_ai_executor();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "ai_executor");
    }

    #[test]
    fn test_executor_factory_create_ai_executor_with_config() {
        let provider = Arc::new(MockProvider);
        let factory = ExecutorFactory::new().with_provider(provider);
        let config = AiExecutorConfig {
            max_tokens: 2048,
            ..Default::default()
        };
        let result = factory.create_ai_executor_with_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_executor_factory_create_validate_executor_with_ai_no_provider() {
        let factory = ExecutorFactory::new();
        let config = ValidateExecutorConfig {
            enable_ai_validation: true,
            ..Default::default()
        };
        let result = factory.create_validate_executor_with_ai(config);
        match result {
            Err(e) => assert!(e.to_string().contains("Provider not configured")),
            Ok(_) => panic!("Should fail without provider"),
        }
    }

    #[test]
    fn test_executor_factory_create_executor_for_task() {
        let factory = ExecutorFactory::new();

        // Test various task types
        assert!(factory.create_executor_for_task("condition").is_ok());
        assert!(factory.create_executor_for_task("branch").is_ok());
        assert!(factory.create_executor_for_task("validate").is_ok());
        assert!(factory.create_executor_for_task("check").is_ok());
        assert!(factory.create_executor_for_task("tool").is_ok());
        assert!(factory.create_executor_for_task("bash").is_ok());

        // Unknown task type should fail
        assert!(factory.create_executor_for_task("unknown").is_err());
    }

    #[test]
    fn test_executor_factory_create_executor_for_ai_task_without_provider() {
        let factory = ExecutorFactory::new();

        // AI task types require provider
        assert!(factory.create_executor_for_task("ai").is_err());
        assert!(factory.create_executor_for_task("claude").is_err());
        assert!(factory.create_executor_for_task("gpt").is_err());
    }

    #[test]
    fn test_executor_factory_create_executor_for_ai_task_with_provider() {
        let provider = Arc::new(MockProvider);
        let factory = ExecutorFactory::new().with_provider(provider);

        assert!(factory.create_executor_for_task("ai").is_ok());
        assert!(factory.create_executor_for_task("claude").is_ok());
        assert!(factory.create_executor_for_task("gpt").is_ok());
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    fn create_task_node(
        id: &str,
        task: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> NodeDef {
        NodeDef {
            id: id.to_string(),
            node_type: NodeType::Task,
            name: format!("{} Node", id),
            description: None,
            task: Some(task.to_string()),
            params,
            on_failure: FailureStrategy::Abort,
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        }
    }

    fn create_condition_node(id: &str, branches: Vec<BranchDef>) -> NodeDef {
        NodeDef {
            id: id.to_string(),
            node_type: NodeType::Condition,
            name: format!("{} Condition", id),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::Abort,
            timeout_ms: None,
            branches: Some(branches),
            parallel_branches: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        }
    }
}