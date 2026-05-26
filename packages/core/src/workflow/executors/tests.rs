//! Executors Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ChatRequest, ChatResponse, ContentBlock, StopReason, Usage};
    use crate::workflow::context::WorkflowContext;
    use crate::workflow::def::{BranchDef, FailureStrategy, NodeDef, NodeType};
    use crate::workflow::rule_engine::Rule;
    use crate::tools::bash::BashTool;
    use std::collections::HashMap;
    use serde_json::json;

    // ============================================================================
    // Mock Provider for AI Executor Testing
    // ============================================================================

    struct MockProvider;

    #[async_trait::async_trait]
    impl crate::providers::Provider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: vec![ContentBlock::Text { text: "{\"result\": \"mock response\"}".to_string() }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        fn clone_box(&self) -> Box<dyn crate::providers::Provider> {
            Box::new(MockProvider)
        }

        fn clone_arc(&self) -> std::sync::Arc<dyn crate::providers::Provider> {
            std::sync::Arc::new(MockProvider)
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

    #[async_trait::async_trait]
    impl crate::providers::Provider for ConfigurableMockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: vec![ContentBlock::Text { text: self.response_text.clone() }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        fn clone_box(&self) -> Box<dyn crate::providers::Provider> {
            Box::new(ConfigurableMockProvider {
                response_text: self.response_text.clone(),
                response_json: self.response_json.clone(),
            })
        }

        fn clone_arc(&self) -> std::sync::Arc<dyn crate::providers::Provider> {
            std::sync::Arc::new(ConfigurableMockProvider {
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
        let provider = std::sync::Arc::new(MockProvider);
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
        let provider = std::sync::Arc::new(MockProvider);
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
        let provider = std::sync::Arc::new(ConfigurableMockProvider::with_json(json!({
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
        let provider = std::sync::Arc::new(MockProvider);
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
        let provider = std::sync::Arc::new(MockProvider);
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
        let tools: Vec<Box<dyn crate::tools::Tool>> = vec![Box::new(BashTool)];
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
        let tools: Vec<Box<dyn crate::tools::Tool>> = vec![Box::new(BashTool)];
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
        let provider = std::sync::Arc::new(ConfigurableMockProvider::with_json(json!({
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
        let provider = std::sync::Arc::new(ConfigurableMockProvider::with_json(json!({
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
        let provider = std::sync::Arc::new(ConfigurableMockProvider::with_json(json!({
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

        let executors: Vec<std::sync::Arc<dyn NodeExecutor>> = vec![
            std::sync::Arc::new(ValidateExecutor::new()),
            std::sync::Arc::new(ValidateExecutor::new()),
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

        let executors: Vec<std::sync::Arc<dyn NodeExecutor>> = vec![
            std::sync::Arc::new(ValidateExecutor::new()),
            std::sync::Arc::new(ValidateExecutor::new()),
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

        let executors: Vec<std::sync::Arc<dyn NodeExecutor>> = vec![
            std::sync::Arc::new(ValidateExecutor::new()),
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

        composite.add_executor(std::sync::Arc::new(ValidateExecutor::new()));
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
        let provider = std::sync::Arc::new(MockProvider);
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
        let provider = std::sync::Arc::new(MockProvider);
        let factory = ExecutorFactory::new().with_provider(provider);
        let result = factory.create_ai_executor();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "ai_executor");
    }

    #[test]
    fn test_executor_factory_create_ai_executor_with_config() {
        let provider = std::sync::Arc::new(MockProvider);
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
        let provider = std::sync::Arc::new(MockProvider);
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