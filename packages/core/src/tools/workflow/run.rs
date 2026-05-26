//! Workflow Execution Tool
//!
//! 让 AI 执行工作流

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::{WorkflowRegistry, WorkflowEngine, WorkflowPersistence, WorkflowStatus};
use crate::workflow::executors::ExecutorFactory;
use crate::providers::Provider;
use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool to run a workflow
pub struct WorkflowRunTool {
    /// Provider 实例（可选，用于 AI-powered 工具）
    provider: Option<Arc<dyn Provider>>, 
}

impl WorkflowRunTool {
    /// 创建新的 WorkflowRunTool（无 Provider）
    pub fn new() -> Self {
        Self { provider: None }
    }
    
    /// 创建带 Provider 的 WorkflowRunTool
    pub fn with_provider(provider: Arc<dyn Provider>) -> Self {
        Self { provider: Some(provider) }
    }
    
    /// 从配置创建 Provider（用于工具执行时自动获取）
    fn create_provider_from_config(&self) -> Result<Arc<dyn Provider>> {
        use crate::providers::ProviderType;
        
        let config = Config::load();
        
        // 获取 API key
        let api_key = config.api_key.clone()
            .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
            .or_else(|| std::env::var("API_KEY").ok())
            .ok_or_else(|| anyhow::anyhow!("未配置 API key，无法执行 AI 任务"))?;
        
        // 获取模型
        let model = config.model.clone()
            .or_else(|| std::env::var("MODEL").ok())
            .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        
        // 解析 provider 类型
        let provider_type = config.provider.clone()
            .or_else(|| std::env::var("PROVIDER").ok())
            .map(|p| match p.to_lowercase().as_str() {
                "openai" => ProviderType::OpenAI,
                _ => ProviderType::Anthropic,
            })
            .unwrap_or_else(|| {
                // 从模型名推断
                if model.starts_with("gpt") || model.starts_with("o1") {
                    ProviderType::OpenAI
                } else {
                    ProviderType::Anthropic
                }
            });
        
        // 获取 base URL
        let base_url = config.base_url.clone()
            .or_else(|| std::env::var("BASE_URL").ok())
            .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok());
        
        crate::providers::create_provider_with_headers(
            provider_type,
            api_key,
            model,
            base_url,
            config.extra_headers.clone()
        ).map(|p| Arc::from(p))
    }
}

impl Default for WorkflowRunTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WorkflowRunTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_run".to_string(),
            description: "执行指定的 workflow。传入 workflow ID 和可选的输入参数，workflow 会按定义的节点顺序执行。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "要执行的 workflow ID。先用 workflow_discover 查看可用 ID。"
                    },
                    "inputs": {
                        "type": "object",
                        "description": "workflow 输入参数（JSON 对象）。键名必须匹配 workflow 的 required_inputs。"
                    }
                },
                "required": ["workflow_id"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let workflow_id = params.get("workflow_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 workflow_id 参数"))?;

        let inputs: HashMap<String, Value> = params.get("inputs")
            .and_then(|v| v.as_object())
            .map(|m| m.clone().into_iter().collect())
            .unwrap_or_default();

        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        // Load workflow
        let workflow_def = registry.load_workflow(workflow_id)?
            .ok_or_else(|| anyhow::anyhow!("Workflow '{}' 不存在。用 workflow_discover 查看可用列表。", workflow_id))?;

        // Get provider - from instance or create from config
        let provider = if let Some(p) = &self.provider {
            log::info!("WorkflowRunTool: using injected provider for model {}", p.model_name());
            eprintln!("[DEBUG] WorkflowRunTool: using injected provider for model {}", p.model_name());
            p.clone()
        } else {
            log::info!("WorkflowRunTool: no injected provider, creating from config");
            eprintln!("[DEBUG] WorkflowRunTool: no injected provider, creating from config");
            // Try to create provider from config
            self.create_provider_from_config()? 
        };
        // Create engine with executor factory
        let factory = ExecutorFactory::new().with_provider(provider);
        let engine = WorkflowEngine::new(workflow_def)?
            .with_executor_factory(factory);
        
        let context = engine.run(inputs).await?;

        // Save context
        let persistence = WorkflowPersistence::new(project_path.as_ref());
        if let Err(e) = persistence.save(&context) {
            log::warn!("Failed to save workflow context: {}", e);
        }

        // Build result
        let status = if context.status == WorkflowStatus::Completed {
            "✓ 完成".to_string()
        } else if context.status == WorkflowStatus::Failed {
            format!("❌ 失败: {}", context.error.unwrap_or_default())
        } else {
            format!("状态: {:?}", context.status)
        };

        Ok(format!(
            "Workflow '{}' 执行结果:\n\n实例ID: {}\n节点执行: {} 个\n{}\n\n变量输出: {}",
            workflow_id,
            context.instance_id,
            context.execution_path.len(),
            status,
            serde_json::to_string_pretty(&context.variables).unwrap_or_default()
        ))
    }
}