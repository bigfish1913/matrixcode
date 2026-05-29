//! Workflow Engine - State Machine Implementation
//!
//! 工作流引擎，实现状态机的基础结构和主运行循环。

use super::context::WorkflowContext;
use super::def::{FailureStrategy, NodeDef, NodeType, WorkflowDef};
use super::executors::{ExecutorFactory, NodeExecutor};
use super::rule_engine::evaluate_expression;
use super::template::TemplateRenderer;
use crate::tools::toolproxy::{ProxyToolDef, ProxyToolExecutor};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// 任务执行器 trait
#[async_trait::async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务，返回输出数据
    async fn execute(
        &self,
        task_name: &str,
        params: &HashMap<String, serde_json::Value>,
        context: &WorkflowContext,
    ) -> Result<serde_json::Value>;
}

/// 工作流事件
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    /// 工作流开始
    Started,
    /// 节点开始执行
    NodeStarted { node_id: String },
    /// 节点执行完成
    NodeCompleted {
        node_id: String,
        output: Option<serde_json::Value>,
    },
    /// 节点执行失败
    NodeFailed { node_id: String, error: String },
    /// 节点跳过
    NodeSkipped { node_id: String, reason: String },
    /// 工作流完成
    Completed,
    /// 工作流失败
    Failed { error: String },
    /// 工作流暂停
    Paused,
    /// 工作流恢复
    Resumed,
}

/// 事件监听器 trait
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: WorkflowEvent);
}

/// 工作流引擎
pub struct WorkflowEngine {
    /// 工作流定义
    definition: WorkflowDef,
    /// 任务执行器（旧接口）
    executor: Option<Arc<dyn TaskExecutor>>,
    /// 节点执行器（新接口）
    node_executors: HashMap<String, Arc<dyn NodeExecutor>>,
    /// 执行器工厂
    executor_factory: Option<ExecutorFactory>,
    /// 代理工具执行器
    proxy_executor: Option<Arc<dyn ProxyToolExecutor>>,
    /// 代理工具定义列表
    proxy_tool_defs: Vec<ProxyToolDef>,
    /// 事件监听器
    listeners: Vec<Box<dyn EventListener>>,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl WorkflowEngine {
    /// 创建新的工作流引擎
    pub fn new(definition: WorkflowDef) -> Result<Self> {
        definition
            .validate()
            .with_context(|| "Invalid workflow definition")?;

        Ok(Self {
            definition,
            executor: None,
            node_executors: HashMap::new(),
            executor_factory: None,
            proxy_executor: None,
            proxy_tool_defs: Vec::new(),
            listeners: Vec::new(),
            template_renderer: TemplateRenderer::new(),
        })
    }

    /// 设置任务执行器（旧接口）
    pub fn with_executor(mut self, executor: Arc<dyn TaskExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// 设置执行器工厂
    pub fn with_executor_factory(mut self, factory: ExecutorFactory) -> Self {
        self.executor_factory = Some(factory);
        self
    }

    /// 设置代理工具执行器
    pub fn with_proxy_executor(
        mut self,
        executor: Arc<dyn ProxyToolExecutor>,
        tool_defs: Vec<ProxyToolDef>,
    ) -> Self {
        self.proxy_executor = Some(executor);
        self.proxy_tool_defs = tool_defs;
        self
    }

    /// 注册节点执行器
    pub fn register_node_executor(
        mut self,
        task_type: &str,
        executor: Arc<dyn NodeExecutor>,
    ) -> Self {
        self.node_executors.insert(task_type.to_string(), executor);
        self
    }

    /// 添加事件监听器
    pub fn add_listener(&mut self, listener: Box<dyn EventListener>) {
        self.listeners.push(listener);
    }

    /// 触发事件
    fn emit_event(&self, event: WorkflowEvent) {
        for listener in &self.listeners {
            listener.on_event(event.clone());
        }
    }

    /// 获取节点执行器
    fn get_node_executor(&self, node: &NodeDef) -> Option<Arc<dyn NodeExecutor>> {
        // 优先从注册的执行器中查找
        if let Some(task) = &node.task
            && let Some(executor) = self.node_executors.get(task)
        {
            return Some(executor.clone());
        }

        // 检查是否是代理工具
        if let Some(task) = &node.task
            && self
                .proxy_tool_defs
                .iter()
                .any(|t| t.definition.name == *task)
            && let Some(executor) = &self.proxy_executor
        {
            return Some(Arc::new(super::executors::ProxyExecutor::new(
                executor.clone(),
                self.proxy_tool_defs.clone(),
            )));
        }

        // 根据节点类型选择默认执行器
        match node.node_type {
            NodeType::Task => {
                // 尝试从工厂创建
                if let Some(factory) = &self.executor_factory
                    && let Some(task) = &node.task
                {
                    // 根据任务名称推断执行器类型
                    // ai / ai_* / claude* / gpt* 使用 AI 执行器
                    let task_lower = task.to_lowercase();
                    if task_lower == "ai"
                        || task_lower.starts_with("ai_")
                        || task_lower.starts_with("claude")
                        || task_lower.starts_with("gpt")
                    {
                        return factory.create_ai_executor().ok();
                    }
                    // 默认使用工具执行器
                    return Some(factory.create_tool_executor());
                }
            }
            NodeType::Condition => {
                if let Some(factory) = &self.executor_factory {
                    return Some(factory.create_condition_executor());
                }
            }
            NodeType::Approval => {
                // 审批节点使用特殊的验证执行器
                if let Some(factory) = &self.executor_factory {
                    return Some(factory.create_validate_executor());
                }
            }
            _ => {}
        }

        None
    }

    /// 运行工作流
    pub async fn run(&self, inputs: HashMap<String, serde_json::Value>) -> Result<WorkflowContext> {
        // 创建上下文
        let mut context = WorkflowContext::new(self.definition.id.clone(), inputs.clone());

        // 验证必填输入
        self.validate_inputs(&context)?;

        // 初始化变量：先添加 inputs
        for (key, value) in inputs {
            context.set_variable(key.clone(), value.clone());
        }

        // 渲染并添加 workflow 定义的变量
        let renderer = crate::workflow::template::TemplateRenderer::new();
        for (key, value) in &self.definition.variables {
            // 如果是字符串，渲染模板
            let rendered_value = if let serde_json::Value::String(s) = value {
                match renderer.render(s, &context.variables) {
                    Ok(rendered) => serde_json::Value::String(rendered),
                    Err(_) => value.clone(), // 渲染失败保持原值
                }
            } else {
                value.clone()
            };
            context.set_variable(key.clone(), rendered_value);
        }

        // 开始工作流
        context.start();
        self.emit_event(WorkflowEvent::Started);

        // 获取开始节点
        let start_node = self
            .definition
            .get_start_node()
            .ok_or_else(|| anyhow::anyhow!("No start node found"))?;

        // 执行工作流
        match self.execute_from_node(start_node, &mut context).await {
            Ok(()) => {
                context.complete();
                self.emit_event(WorkflowEvent::Completed);
            }
            Err(e) => {
                context.fail(e.to_string());
                self.emit_event(WorkflowEvent::Failed {
                    error: e.to_string(),
                });
            }
        }

        Ok(context)
    }

    /// 从指定节点开始执行
    async fn execute_from_node(&self, node: &NodeDef, context: &mut WorkflowContext) -> Result<()> {
        let mut current_node = Some(node);

        while let Some(node) = current_node {
            // 检查工作流状态
            if !context.can_continue() {
                break;
            }

            // 执行节点
            match self.execute_node(node, context).await {
                Ok(next_node_id) => {
                    current_node = next_node_id
                        .as_ref()
                        .and_then(|id| self.definition.get_node(id));
                }
                Err(e) => {
                    // 处理失败
                    match &node.on_failure {
                        FailureStrategy::Retry {
                            max_attempts,
                            interval_ms,
                        } => {
                            let exec = context.get_or_create_node_execution(&node.id);
                            if exec.retry_count < *max_attempts {
                                exec.increment_retry();
                                if let Some(interval) = interval_ms {
                                    tokio::time::sleep(Duration::from_millis(*interval)).await;
                                }
                                continue; // 重试当前节点
                            } else {
                                return Err(e);
                            }
                        }
                        FailureStrategy::Ignore => {
                            // 忽略错误，标记节点为 Skipped 并继续执行下一个节点
                            let exec = context.get_or_create_node_execution(&node.id);
                            exec.skip();
                            self.emit_event(WorkflowEvent::NodeSkipped {
                                node_id: node.id.clone(),
                                reason: e.to_string(),
                            });
                            let next = self.get_next_node(node, context)?;
                            current_node =
                                next.as_ref().and_then(|id| self.definition.get_node(id));
                        }
                        FailureStrategy::Abort => {
                            return Err(e);
                        }
                        FailureStrategy::Goto { target } => {
                            current_node = self.definition.get_node(target);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 执行单个节点
    async fn execute_node(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<Option<String>> {
        // 创建执行记录
        let execution = context.get_or_create_node_execution(&node.id);
        execution.start();
        self.emit_event(WorkflowEvent::NodeStarted {
            node_id: node.id.clone(),
        });

        // 设置当前节点
        context.set_current_node(node.id.clone());

        // 处理超时
        let result = if let Some(timeout_ms) = node.timeout_ms {
            timeout(
                Duration::from_millis(timeout_ms),
                self.execute_node_inner(node, context),
            )
            .await
            .with_context(|| format!("Node '{}' timed out after {}ms", node.id, timeout_ms))?
        } else {
            self.execute_node_inner(node, context).await
        };

        match result {
            Ok(output) => {
                let exec = context.get_or_create_node_execution(&node.id);
                exec.complete(output.clone());
                self.emit_event(WorkflowEvent::NodeCompleted {
                    node_id: node.id.clone(),
                    output,
                });

                // 获取下一个节点
                self.get_next_node(node, context)
            }
            Err(e) => {
                let exec = context.get_or_create_node_execution(&node.id);
                exec.fail(e.to_string());
                self.emit_event(WorkflowEvent::NodeFailed {
                    node_id: node.id.clone(),
                    error: e.to_string(),
                });
                Err(e)
            }
        }
    }

    /// 节点内部执行逻辑
    async fn execute_node_inner(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        match &node.node_type {
            NodeType::Start => Ok(None),
            NodeType::End => Ok(None),
            NodeType::Task => self.execute_task(node, context).await,
            NodeType::Condition => self.execute_condition(node, context).await,
            NodeType::Parallel => self.execute_parallel(node, context).await,
            NodeType::SubWorkflow => self.execute_subworkflow(node, context).await,
            NodeType::Wait => self.execute_wait(node, context).await,
            NodeType::Approval => self.execute_approval(node, context).await,
        }
    }

    /// 执行任务节点
    async fn execute_task(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let task_name = node
            .task
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Task node '{}' has no task name", node.id))?;

        // 渲染参数
        let mut rendered_params = HashMap::new();
        for (key, value) in &node.params {
            if let serde_json::Value::String(s) = value {
                let rendered = self.template_renderer.render(s, &context.variables)?;
                rendered_params.insert(key.clone(), serde_json::Value::String(rendered));
            } else {
                rendered_params.insert(key.clone(), value.clone());
            }
        }

        // 尝试使用新的 NodeExecutor 接口
        if let Some(node_executor) = self.get_node_executor(node) {
            let output = node_executor.execute(node, context).await?;
            return Ok(Some(output));
        }

        // 回退到旧的 TaskExecutor 接口
        if let Some(executor) = &self.executor {
            let output = executor
                .execute(task_name, &rendered_params, context)
                .await?;
            Ok(Some(output))
        } else {
            // 无执行器，返回模拟输出
            Ok(Some(
                serde_json::json!({ "task": task_name, "status": "completed" }),
            ))
        }
    }

    /// 执行条件节点
    async fn execute_condition(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let branches = node
            .branches
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Condition node '{}' has no branches", node.id))?;

        for branch in branches {
            if evaluate_expression(&branch.condition, &context.variables)? {
                // 找到匹配的分支，设置目标节点
                return Ok(Some(serde_json::Value::String(branch.target.clone())));
            }
        }

        // 没有匹配的分支
        Ok(None)
    }

    /// 执行并行节点
    async fn execute_parallel(
        &self,
        node: &NodeDef,
        _context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let branches = node
            .parallel_branches
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Parallel node '{}' has no branches", node.id))?;

        // 并行执行所有分支
        let mut outputs = Vec::new();
        for branch in branches {
            // 这里简化处理，实际应该并行执行
            outputs.push(serde_json::json!({
                "branch": branch.name,
                "status": "completed"
            }));
        }

        Ok(Some(serde_json::Value::Array(outputs)))
    }

    /// 执行子工作流
    async fn execute_subworkflow(
        &self,
        node: &NodeDef,
        _context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let workflow_name = node.workflow.as_ref().ok_or_else(|| {
            anyhow::anyhow!("SubWorkflow node '{}' has no workflow name", node.id)
        })?;

        // 这里简化处理，实际应该加载并执行子工作流
        Ok(Some(serde_json::json!({
            "workflow": workflow_name,
            "status": "completed"
        })))
    }

    /// 执行等待节点
    async fn execute_wait(
        &self,
        node: &NodeDef,
        _context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let wait_ms = node.wait_ms.unwrap_or(0);
        if wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        }
        Ok(None)
    }

    /// 执行审批节点
    async fn execute_approval(
        &self,
        node: &NodeDef,
        _context: &mut WorkflowContext,
    ) -> Result<Option<serde_json::Value>> {
        let approvers = node
            .approvers
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Approval node '{}' has no approvers", node.id))?;

        // 这里简化处理，实际应该等待审批
        Ok(Some(serde_json::json!({
            "approvers": approvers,
            "status": "pending_approval"
        })))
    }

    /// 获取下一个节点
    fn get_next_node(&self, node: &NodeDef, context: &WorkflowContext) -> Result<Option<String>> {
        // 结束节点没有下一个节点
        if node.node_type == NodeType::End {
            return Ok(None);
        }

        // 获取输出边
        let edges = self.definition.get_outgoing_edges(&node.id);

        if edges.is_empty() {
            return Ok(None);
        }

        // 条件节点从分支获取下一个节点
        if node.node_type == NodeType::Condition {
            let exec = context.get_node_execution(&node.id);
            if let Some(exec) = exec
                && let Some(serde_json::Value::String(target)) = &exec.output
            {
                return Ok(Some(target.clone()));
            }
        }

        // 根据边条件选择下一个节点
        for edge in edges {
            if let Some(condition) = &edge.condition {
                if evaluate_expression(condition, &context.variables)? {
                    return Ok(Some(edge.to.clone()));
                }
            } else {
                // 无条件的边，直接返回
                return Ok(Some(edge.to.clone()));
            }
        }

        // 没有匹配的边
        Ok(None)
    }

    /// 验证输入参数
    fn validate_inputs(&self, context: &WorkflowContext) -> Result<()> {
        for input_def in &self.definition.inputs {
            if input_def.required
                && context.get_input(&input_def.name).is_none()
                && input_def.default.is_none()
            {
                anyhow::bail!("Required input '{}' is missing", input_def.name);
            }
        }
        Ok(())
    }

    /// 获取工作流定义
    pub fn definition(&self) -> &WorkflowDef {
        &self.definition
    }
}

/// 默认任务执行器（用于测试）
pub struct DefaultTaskExecutor;

#[async_trait::async_trait]
impl TaskExecutor for DefaultTaskExecutor {
    async fn execute(
        &self,
        task_name: &str,
        _params: &HashMap<String, serde_json::Value>,
        _context: &WorkflowContext,
    ) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "task": task_name,
            "status": "completed",
            "output": null
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::WorkflowStatus;
    use super::super::def::EdgeDef;
    use super::*;

    fn create_simple_workflow() -> WorkflowDef {
        WorkflowDef {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            nodes: vec![
                NodeDef {
                    id: "start".to_string(),
                    node_type: NodeType::Start,
                    name: "Start".to_string(),
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
                },
                NodeDef {
                    id: "task1".to_string(),
                    node_type: NodeType::Task,
                    name: "Task 1".to_string(),
                    description: None,
                    task: Some("do_something".to_string()),
                    params: HashMap::new(),
                    on_failure: FailureStrategy::Abort,
                    timeout_ms: None,
                    branches: None,
                    parallel_branches: None,
                    workflow: None,
                    wait_ms: None,
                    approvers: None,
                },
                NodeDef {
                    id: "end".to_string(),
                    node_type: NodeType::End,
                    name: "End".to_string(),
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
                },
            ],
            edges: vec![
                EdgeDef {
                    id: "e1".to_string(),
                    from: "start".to_string(),
                    to: "task1".to_string(),
                    condition: None,
                    label: None,
                },
                EdgeDef {
                    id: "e2".to_string(),
                    from: "task1".to_string(),
                    to: "end".to_string(),
                    condition: None,
                    label: None,
                },
            ],
            variables: HashMap::new(),
            default_failure_strategy: FailureStrategy::Abort,
            timeout_ms: None,
        }
    }

    #[tokio::test]
    async fn test_engine_run() {
        let workflow = create_simple_workflow();
        let engine = WorkflowEngine::new(workflow).unwrap();

        let inputs = HashMap::new();
        let context = engine.run(inputs).await.unwrap();

        assert_eq!(context.status, WorkflowStatus::Completed);
        assert_eq!(context.execution_path.len(), 3);
    }

    #[tokio::test]
    async fn test_engine_with_executor() {
        let workflow = create_simple_workflow();
        let executor = Arc::new(DefaultTaskExecutor);
        let engine = WorkflowEngine::new(workflow)
            .unwrap()
            .with_executor(executor);

        let inputs = HashMap::new();
        let context = engine.run(inputs).await.unwrap();

        assert_eq!(context.status, WorkflowStatus::Completed);
    }
}
