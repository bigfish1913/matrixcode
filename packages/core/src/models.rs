use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constants::{DEFAULT_MAX_TOKENS, COMPRESS_MAX_TOKENS, FAST_MAX_TOKENS};
use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role,
};

/// Default model names for different roles.
pub const DEFAULT_MAIN_MODEL: &str = "claude-sonnet-4-20250514";
pub const DEFAULT_PLAN_MODEL: &str = "claude-sonnet-4-20250514";
pub const DEFAULT_COMPRESS_MODEL: &str = "claude-3-5-haiku-20241022";
pub const DEFAULT_FAST_MODEL: &str = "claude-3-5-haiku-20241022";

/// Model role - what purpose this model serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    /// Main model for task execution.
    Main,
    /// Planning model for task decomposition.
    Plan,
    /// Compression model for context summarization.
    Compress,
    /// Fast model for quick operations.
    Fast,
}

impl ModelRole {
    pub fn default_model(&self) -> &'static str {
        match self {
            ModelRole::Main => DEFAULT_MAIN_MODEL,
            ModelRole::Plan => DEFAULT_PLAN_MODEL,
            ModelRole::Compress => DEFAULT_COMPRESS_MODEL,
            ModelRole::Fast => DEFAULT_FAST_MODEL,
        }
    }
}

/// Configuration for a single model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model name/identifier.
    pub name: String,
    /// Maximum output tokens.
    pub max_tokens: u32,
    /// Whether to enable thinking for this model.
    pub think: bool,
    /// Estimated context window size.
    pub context_size: Option<u32>,
}

impl ModelConfig {
    pub fn new(name: String) -> Self {
        Self {
            name: name.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            think: true,
            context_size: infer_context_size(&name),
        }
    }

    /// Create with specific role defaults.
    pub fn for_role(role: ModelRole) -> Self {
        let name = role.default_model().to_string();
        match role {
            ModelRole::Main => Self::new(name),
            ModelRole::Plan => Self::new(name),
            ModelRole::Compress => Self {
                name,
                max_tokens: COMPRESS_MAX_TOKENS,
                think: false,
                context_size: Some(200_000),
            },
            ModelRole::Fast => Self {
                name,
                max_tokens: FAST_MAX_TOKENS,
                think: false,
                context_size: Some(200_000),
            },
        }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }
}

/// Infer context window size from model name.
/// Honours the `CONTEXT_SIZE` env variable first so users can override.
pub fn context_window_for(model: &str) -> Option<u32> {
    // Allow user override via environment variable
    if let Ok(raw) = std::env::var("CONTEXT_SIZE")
        && let Ok(n) = raw.trim().parse::<u32>()
        && n > 0
    {
        return Some(n);
    }

    let m = model.to_ascii_lowercase();

    // Anthropic models - 1M context window
    if m.contains("1m") || m.contains("opus-4-7") || m.contains("opus-4.7") {
        return Some(1_000_000);
    }
    if m.contains("claude-3")
        || m.contains("claude-4")
        || m.contains("claude-opus")
        || m.contains("claude-sonnet")
        || m.contains("claude-haiku")
    {
        return Some(200_000);
    }
    if m.contains("claude-2") || m.contains("claude-instant") {
        return Some(100_000);
    }

    // OpenAI models
    if m.contains("gpt-4o") || m.contains("gpt-4-turbo") {
        return Some(128_000);
    }
    if m.contains("o1") || m.contains("o3") || m.contains("o4") {
        return Some(200_000);
    }
    if m.contains("gpt-4-32k") {
        return Some(32_768);
    }
    if m.contains("gpt-4") && !m.contains("turbo") && !m.contains("o") {
        return Some(8_192);
    }
    if m.contains("gpt-3.5-turbo-16k") {
        return Some(16_384);
    }
    if m.contains("gpt-3.5") {
        return Some(4_096);
    }

    // DeepSeek models
    if m.contains("deepseek-v3") || m.contains("deepseek-r1") {
        return Some(128_000);
    }
    if m.contains("deepseek") {
        return Some(64_000);
    }

    // Kimi models
    if m.contains("kimi") {
        return Some(128_000);
    }

    // Qwen models
    if m.contains("qwen") {
        if m.contains("qwen-max") || m.contains("qwen2.5-72b") || m.contains("qwen2.5") {
            return Some(128_000);
        }
        if m.contains("qwen2") {
            return Some(32_000);
        }
        return Some(8_192);
    }

    // Llama models
    if m.contains("llama-3") || m.contains("llama3") {
        if m.contains("70b") || m.contains("405b") {
            return Some(128_000);
        }
        return Some(8_192);
    }

    // GLM models (Zhipu AI)
    if m.contains("glm") {
        return Some(128_000);
    }

    None
}

/// Legacy alias for context_window_for (internal use).
fn infer_context_size(model: &str) -> Option<u32> {
    context_window_for(model)
}

/// Multi-model configuration manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModelConfig {
    /// Main model for primary tasks.
    pub main: ModelConfig,
    /// Planning model for task decomposition.
    pub plan: ModelConfig,
    /// Compression model for context summarization.
    pub compress: ModelConfig,
    /// Fast model for quick operations.
    pub fast: ModelConfig,
}

impl Default for MultiModelConfig {
    fn default() -> Self {
        Self {
            main: ModelConfig::for_role(ModelRole::Main),
            plan: ModelConfig::for_role(ModelRole::Plan),
            compress: ModelConfig::for_role(ModelRole::Compress),
            fast: ModelConfig::for_role(ModelRole::Fast),
        }
    }
}

impl MultiModelConfig {
    /// Create with a main model, all other roles also use this model by default.
    /// This ensures that if no specific model is configured for a role, it falls back to the main model.
    pub fn with_main(main_model: String) -> Self {
        let main_config = ModelConfig::new(main_model);
        Self {
            main: main_config.clone(),
            plan: main_config.clone(),
            compress: main_config.clone(),
            fast: main_config,
        }
    }

    /// Create where all roles use the same model.
    pub fn unified(model: String) -> Self {
        let config = ModelConfig::new(model);
        Self {
            main: config.clone(),
            plan: config.clone(),
            compress: config.clone(),
            fast: config,
        }
    }

    /// Get config for a specific role.
    pub fn get(&self, role: ModelRole) -> &ModelConfig {
        match role {
            ModelRole::Main => &self.main,
            ModelRole::Plan => &self.plan,
            ModelRole::Compress => &self.compress,
            ModelRole::Fast => &self.fast,
        }
    }

    /// Set model for a specific role.
    pub fn set(&mut self, role: ModelRole, config: ModelConfig) {
        match role {
            ModelRole::Main => self.main = config,
            ModelRole::Plan => self.plan = config,
            ModelRole::Compress => self.compress = config,
            ModelRole::Fast => self.fast = config,
        }
    }

    /// Format for display.
    pub fn format_summary(&self) -> String {
        format!(
            "main: {}, plan: {}, compress: {}, fast: {}",
            self.main.display_name(),
            self.plan.display_name(),
            self.compress.display_name(),
            self.fast.display_name()
        )
    }
}

/// Task complexity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Simple,
    Moderate,
    Complex,
}

impl TaskComplexity {
    pub fn display(&self) -> &'static str {
        match self {
            TaskComplexity::Simple => "简单",
            TaskComplexity::Moderate => "中等",
            TaskComplexity::Complex => "复杂",
        }
    }
}

/// Step difficulty level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepDifficulty {
    Easy,
    Medium,
    Hard,
}

/// A single step in the task plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step description.
    pub description: String,
    /// Tools needed for this step.
    pub tools: Vec<String>,
    /// Whether this step is optional.
    pub optional: bool,
}

/// Task plan generated by the planning model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Original user request.
    pub request: String,
    /// Decomposed steps.
    pub steps: Vec<PlanStep>,
    /// Estimated complexity.
    pub complexity: TaskComplexity,
    /// Suggested approach summary.
    pub approach: String,
    /// Potential risks or considerations.
    pub considerations: Vec<String>,
}

impl TaskPlan {
    /// Format for display.
    pub fn format(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("任务分析: {}\n", self.request));
        output.push_str(&format!("复杂度: {}\n", self.complexity.display()));
        output.push_str(&format!("建议方案: {}\n\n", self.approach));

        output.push_str("执行步骤:\n");
        for (i, step) in self.steps.iter().enumerate() {
            let marker = if step.optional { "[可选]" } else { "" };
            output.push_str(&format!("{}. {} {}\n", i + 1, step.description, marker));
            if !step.tools.is_empty() {
                output.push_str(&format!("   工具: {}\n", step.tools.join(", ")));
            }
        }

        if !self.considerations.is_empty() {
            output.push_str("\n注意事项:\n");
            for c in &self.considerations {
                output.push_str(&format!("• {}\n", c));
            }
        }

        output
    }

    /// Convert to todo items for the agent.
    pub fn to_todo_items(&self) -> Vec<TodoItem> {
        self.steps
            .iter()
            .enumerate()
            .map(|(i, step)| TodoItem {
                content: step.description.clone(),
                active_form: format!("执行步骤 {}: {}", i + 1, step.description),
                status: if i == 0 {
                    "in_progress".to_string()
                } else {
                    "pending".to_string()
                },
            })
            .collect()
    }
}

/// Todo item for task tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub active_form: String,
    pub status: String,
}

/// Planner for generating task plans using the plan model.
pub struct Planner {
    provider: Box<dyn Provider>,
    config: ModelConfig,
}

impl Planner {
    /// Create a new planner.
    pub fn new(provider: Box<dyn Provider>, config: ModelConfig) -> Self {
        Self { provider, config }
    }

    /// Generate a task plan for the given request.
    pub async fn plan(&self, request: &str, available_tools: &[&str]) -> Result<TaskPlan> {
        let prompt = build_plan_prompt(request, available_tools);

        let chat_request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt),
            }],
            tools: vec![],
            system: Some(PLAN_SYSTEM_PROMPT.to_string()),
            think: false,
            max_tokens: self.config.max_tokens,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = self.provider.chat(chat_request).await?;
        let text = extract_text(&response);

        parse_plan_response(request, &text)
    }

    /// Quick complexity assessment using fast model.
    pub async fn assess_complexity(&self, request: &str) -> Result<TaskComplexity> {
        let prompt = format!(
            "评估此任务的复杂度（简单/中等/复杂），只需回答一个词：\n{}",
            request
        );

        let chat_request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt),
            }],
            tools: vec![],
            system: None,
            think: false,
            max_tokens: 50,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = self.provider.chat(chat_request).await?;
        let text = extract_text(&response).to_lowercase();

        if text.contains("简单") || text.contains("simple") {
            Ok(TaskComplexity::Simple)
        } else if text.contains("复杂") || text.contains("complex") {
            Ok(TaskComplexity::Complex)
        } else {
            Ok(TaskComplexity::Moderate)
        }
    }
}

/// System prompt for planning.
const PLAN_SYSTEM_PROMPT: &str = r#"你是一个任务规划助手。你的职责是分析编程任务，并将其分解为清晰的执行步骤。

输出要求（JSON格式）：
```json
{
  "complexity": "simple|moderate|complex",
  "approach": "建议的方案（一句话）",
  "steps": [
    {
      "description": "步骤描述",
      "tools": ["需要的工具"],
      "optional": false
    }
  ],
  "considerations": ["注意事项"]
}
```

规划原则：
1. 简单任务（如读取文件、简单查询）只需1-2步
2. 中等任务（如修改代码、添加功能）需要3-5步
3. 复杂任务（如重构、跨模块修改）需要详细规划
4. 每个步骤要具体、可执行
5. 标记可选步骤和潜在风险"#;

/// Build planning prompt.
fn build_plan_prompt(request: &str, available_tools: &[&str]) -> String {
    format!(
        r#"用户请求：
{}

可用工具：
{}

请分析任务并生成执行计划（JSON格式）。"#,
        request,
        available_tools.join(", ")
    )
}

/// Parse planning response into TaskPlan.
fn parse_plan_response(request: &str, text: &str) -> Result<TaskPlan> {
    // Try to parse as JSON
    if let Some(json_start) = text.find('{')
        && let Some(json_end) = text.rfind('}')
    {
        let json_str = &text[json_start..=json_end];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
            return Ok(TaskPlan {
                request: request.to_string(),
                steps: parse_steps(&parsed["steps"]),
                complexity: parse_complexity(&parsed["complexity"]),
                approach: parsed["approach"]
                    .as_str()
                    .unwrap_or("直接执行")
                    .to_string(),
                considerations: parsed["considerations"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }

    // Fallback: create simple plan from text
    Ok(TaskPlan {
        request: request.to_string(),
        steps: parse_steps_from_text(text),
        complexity: TaskComplexity::Moderate,
        approach: "按步骤执行".to_string(),
        considerations: vec!["请检查执行结果".to_string()],
    })
}

fn parse_steps(value: &serde_json::Value) -> Vec<PlanStep> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    Some(PlanStep {
                        description: v["description"].as_str()?.to_string(),
                        tools: v["tools"]
                            .as_array()
                            .map(|t| {
                                t.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        optional: v["optional"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_complexity(value: &serde_json::Value) -> TaskComplexity {
    match value.as_str().map(|s| s.to_lowercase()) {
        Some(s) if s.contains("simple") || s.contains("简单") => TaskComplexity::Simple,
        Some(s) if s.contains("complex") || s.contains("复杂") => TaskComplexity::Complex,
        _ => TaskComplexity::Moderate,
    }
}

fn parse_steps_from_text(text: &str) -> Vec<PlanStep> {
    text.lines()
        .filter(|l| l.trim().starts_with(|c: char| c.is_ascii_digit()))
        .take(5)
        .map(|l| PlanStep {
            description: l.split_whitespace().skip(1).collect::<Vec<_>>().join(" "),
            tools: vec!["read".to_string()],
            optional: false,
        })
        .collect()
}

fn extract_text(response: &ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_defaults() {
        let main = ModelConfig::for_role(ModelRole::Main);
        assert!(main.name.contains("claude"));
        assert!(main.think);

        let compress = ModelConfig::for_role(ModelRole::Compress);
        assert!(compress.name.contains("haiku"));
        assert!(!compress.think);
    }

    #[test]
    fn test_infer_context_size() {
        assert_eq!(infer_context_size("claude-sonnet-4"), Some(200_000));
        assert_eq!(infer_context_size("gpt-4o"), Some(128_000));
        assert_eq!(infer_context_size("claude-3-5-haiku"), Some(200_000));
        // 1M context models
        assert_eq!(infer_context_size("claude-sonnet-4-1m"), Some(1_000_000));
        assert_eq!(infer_context_size("claude-opus-4-7"), Some(1_000_000));
    }

    #[test]
    fn test_multi_model_config() {
        let config = MultiModelConfig::default();
        assert!(config.main.name.contains("sonnet"));
        assert!(config.compress.name.contains("haiku"));
    }

    #[test]
    fn test_multi_model_config_with_main() {
        // with_main should make all roles use the main model
        let config = MultiModelConfig::with_main("claude-sonnet-4".to_string());

        // All roles should use the same model
        assert_eq!(config.main.name, "claude-sonnet-4");
        assert_eq!(config.plan.name, "claude-sonnet-4");
        assert_eq!(config.compress.name, "claude-sonnet-4");
        assert_eq!(config.fast.name, "claude-sonnet-4");

        // All should have thinking enabled (inherited from main)
        assert!(config.main.think);
        assert!(config.plan.think);
        assert!(config.compress.think);
        assert!(config.fast.think);
    }

    #[test]
    fn test_multi_model_config_override() {
        let mut config = MultiModelConfig::with_main("claude-sonnet-4".to_string());

        // Override compress model
        config.set(
            ModelRole::Compress,
            ModelConfig::new("claude-3-5-haiku".to_string()),
        );

        assert_eq!(config.main.name, "claude-sonnet-4");
        assert_eq!(config.plan.name, "claude-sonnet-4");
        assert_eq!(config.compress.name, "claude-3-5-haiku");
        assert_eq!(config.fast.name, "claude-sonnet-4"); // Still uses main
    }

    #[test]
    fn test_task_plan_format() {
        let plan = TaskPlan {
            request: "测试任务".to_string(),
            steps: vec![PlanStep {
                description: "读取文件".to_string(),
                tools: vec!["read".to_string()],
                optional: false,
            }],
            complexity: TaskComplexity::Simple,
            approach: "直接执行".to_string(),
            considerations: vec!["注意检查".to_string()],
        };

        let formatted = plan.format();
        assert!(formatted.contains("测试任务"));
        assert!(formatted.contains("简单"));
        assert!(formatted.contains("读取文件"));
    }

    #[test]
    fn test_complexity_display() {
        assert_eq!(TaskComplexity::Simple.display(), "简单");
        assert_eq!(TaskComplexity::Moderate.display(), "中等");
        assert_eq!(TaskComplexity::Complex.display(), "复杂");
    }

    #[test]
    fn test_task_plan_to_todo() {
        let plan = TaskPlan {
            request: "任务".to_string(),
            steps: vec![
                PlanStep {
                    description: "步骤1".to_string(),
                    tools: vec![],
                    optional: false,
                },
                PlanStep {
                    description: "步骤2".to_string(),
                    tools: vec![],
                    optional: false,
                },
            ],
            complexity: TaskComplexity::Simple,
            approach: "执行".to_string(),
            considerations: vec![],
        };

        let todos = plan.to_todo_items();
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].status, "in_progress");
        assert_eq!(todos[1].status, "pending");
    }

    #[test]
    fn test_parse_plan_response_json() {
        let json = r#"{"complexity":"simple","approach":"直接读取","steps":[{"description":"read file","tools":["read"],"optional":false}],"considerations":[]}"#;
        let plan = parse_plan_response("test", json).unwrap();

        assert_eq!(plan.complexity, TaskComplexity::Simple);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].description, "read file");
    }
}
