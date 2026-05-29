//! Pre-processing Hook for Skills/Workflows Trigger Detection
//!
//! This module implements the **backend-side** trigger detection that was
//! previously described in the prompt. By moving this logic to code:
//! - Eliminates ambiguity in pattern matching
//! - Provides deterministic behavior
//! - Reduces prompt token cost (~100 lines removed from prompt)
//! - Enables easier testing and debugging

use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

/// Trigger type detection result
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessResult {
    /// A skill was triggered
    SkillTriggered { skill_id: String, confidence: f32 },
    /// A workflow was triggered
    WorkflowTriggered {
        workflow_id: String,
        inputs: HashMap<String, String>,
    },
    /// Continue normal processing
    Continue,
}

/// Type of trigger detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    Skill,
    Workflow,
    SkillKeyword,
    WorkflowKeyword,
}

/// Skill trigger pattern
#[derive(Debug, Clone)]
pub struct SkillPattern {
    /// Skill identifier (e.g., "code-review", "refactor")
    pub skill_id: String,
    /// Primary trigger patterns (regex or keyword)
    pub patterns: Vec<String>,
    /// Compiled regex patterns
    pub compiled: Vec<Regex>,
    /// Confidence weight (0.0 - 1.0)
    pub weight: f32,
}

impl SkillPattern {
    pub fn new(skill_id: impl Into<String>, patterns: Vec<&str>, weight: f32) -> Self {
        let patterns: Vec<String> = patterns.into_iter().map(|s| s.to_string()).collect();
        let compiled = patterns
            .iter()
            .filter_map(|p| Regex::new(&format!("(?i){}", p)).ok())
            .collect();

        Self {
            skill_id: skill_id.into(),
            patterns,
            compiled,
            weight,
        }
    }

    /// Check if user message matches this skill
    pub fn matches(&self, message: &str) -> Option<f32> {
        for regex in &self.compiled {
            if regex.is_match(message) {
                return Some(self.weight);
            }
        }
        None
    }
}

/// Workflow trigger configuration
#[derive(Debug, Clone)]
pub struct WorkflowTrigger {
    /// Workflow identifier
    pub workflow_id: String,
    /// Trigger keywords
    pub keywords: Vec<String>,
    /// Required inputs that can be extracted from message
    pub extractable_inputs: Vec<String>,
}

impl WorkflowTrigger {
    pub fn new(workflow_id: impl Into<String>, keywords: Vec<&str>, inputs: Vec<&str>) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            keywords: keywords.into_iter().map(|s| s.to_string()).collect(),
            extractable_inputs: inputs.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Check if message triggers this workflow
    pub fn matches(&self, message: &str) -> bool {
        let msg_lower = message.to_lowercase();
        self.keywords
            .iter()
            .any(|k| msg_lower.contains(&k.to_lowercase()))
    }

    /// Extract inputs from message (simple extraction)
    pub fn extract_inputs(&self, message: &str) -> HashMap<String, String> {
        let mut inputs = HashMap::new();

        // Simple topic extraction for common patterns
        if self.extractable_inputs.contains(&"topic".to_string()) {
            // Pattern: "generate article about X" or "X article"
            let patterns = [
                r"(?i)(?:generate|create|write).*(?:article|post|content).*?about\s+(.+?)(?:\.|$)",
                r"(?i)(?:article|post|content)\s+about\s+(.+?)(?:\.|$)",
            ];

            for pattern in patterns {
                if let Ok(re) = Regex::new(pattern) {
                    if let Some(caps) = re.captures(message) {
                        if let Some(topic) = caps.get(1) {
                            inputs.insert("topic".to_string(), topic.as_str().trim().to_string());
                            break;
                        }
                    }
                }
            }
        }

        inputs
    }
}

/// Pre-processing hook for trigger detection
pub struct PreProcessHook {
    /// Skill patterns
    skills: Vec<SkillPattern>,
    /// Workflow triggers
    workflows: Vec<WorkflowTrigger>,
    /// Minimum confidence threshold
    confidence_threshold: f32,
}

impl Default for PreProcessHook {
    fn default() -> Self {
        Self::new()
    }
}

impl PreProcessHook {
    /// Create with default patterns (from claude-code-analysis)
    pub fn new() -> Self {
        Self {
            skills: Self::default_skill_patterns(),
            workflows: Self::default_workflow_triggers(),
            confidence_threshold: 0.7,
        }
    }

    /// Default skill patterns based on analysis
    fn default_skill_patterns() -> Vec<SkillPattern> {
        vec![
            // Code review skill
            SkillPattern::new(
                "code-review",
                vec![
                    r"/review",
                    r"审查.*代码",
                    r"检查.*代码",
                    r"code\s*review",
                    r"review.*code",
                ],
                0.9,
            ),
            // Refactor skill
            SkillPattern::new(
                "refactor",
                vec![r"/refactor", r"重构.*代码", r"优化.*结构", r"refactor"],
                0.9,
            ),
            // Debug skill
            SkillPattern::new(
                "debug",
                vec![r"/debug", r"调试.*问题", r"排查.*问题", r"debug", r"调试"],
                0.9,
            ),
            // Planning skill
            SkillPattern::new(
                "planning",
                vec![r"/plan", r"规划.*方案", r"设计.*方案", r"plan"],
                0.9,
            ),
            // Security review skill
            SkillPattern::new(
                "security-review",
                vec![
                    r"/security",
                    r"安全.*审查",
                    r"安全.*检查",
                    r"security\s*review",
                ],
                0.9,
            ),
            // Demo skill
            SkillPattern::new("demo", vec![r"/demo", r"演示", r"demo"], 0.8),
            // Git commit skill
            SkillPattern::new(
                "git-commit",
                vec![r"/commit", r"提交.*代码", r"commit"],
                0.8,
            ),
        ]
    }

    /// Default workflow triggers based on analysis
    fn default_workflow_triggers() -> Vec<WorkflowTrigger> {
        vec![
            // Image article workflow
            WorkflowTrigger::new(
                "image-article",
                vec!["generate article", "生成文章", "create article", "图片文章"],
                vec!["topic"],
            ),
            // Analysis workflow
            WorkflowTrigger::new(
                "code-analysis",
                vec!["analyze code", "分析代码", "代码分析", "code analysis"],
                vec!["target"],
            ),
            // Test workflow
            WorkflowTrigger::new(
                "test-runner",
                vec!["run tests", "运行测试", "执行测试", "test suite"],
                vec!["test_path"],
            ),
        ]
    }

    /// Process user message and detect triggers
    pub fn process(&self, message: &str) -> ProcessResult {
        // Step 1: Check for skill triggers
        for skill in &self.skills {
            if let Some(confidence) = skill.matches(message) {
                if confidence >= self.confidence_threshold {
                    return ProcessResult::SkillTriggered {
                        skill_id: skill.skill_id.clone(),
                        confidence,
                    };
                }
            }
        }

        // Step 2: Check for workflow triggers
        for workflow in &self.workflows {
            if workflow.matches(message) {
                let inputs = workflow.extract_inputs(message);
                return ProcessResult::WorkflowTriggered {
                    workflow_id: workflow.workflow_id.clone(),
                    inputs,
                };
            }
        }

        // Step 3: Continue normal processing
        ProcessResult::Continue
    }

    /// Add a custom skill pattern
    pub fn add_skill(&mut self, skill: SkillPattern) {
        self.skills.push(skill);
    }

    /// Add a custom workflow trigger
    pub fn add_workflow(&mut self, workflow: WorkflowTrigger) {
        self.workflows.push(workflow);
    }

    /// Set confidence threshold
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Check if message contains skill-like patterns (for heuristics)
    pub fn has_skill_intent(&self, message: &str) -> bool {
        let msg_lower = message.to_lowercase();

        // Check for common skill indicators
        let skill_indicators = [
            "review", "refactor", "debug", "plan", "security", "审查", "重构", "调试", "规划",
            "安全",
        ];

        skill_indicators.iter().any(|ind| msg_lower.contains(ind))
    }

    /// Check if message contains workflow-like patterns (multiple steps)
    pub fn has_workflow_intent(&self, message: &str) -> bool {
        let msg_lower = message.to_lowercase();

        // Check for multi-step indicators
        let workflow_indicators = [
            "generate", "create", "analyze", "process", "batch", "生成", "创建", "分析", "处理",
            "批量", "and then", "then", "after", "然后", "接着",
        ];

        // Count how many indicators are present
        let count = workflow_indicators
            .iter()
            .filter(|ind| msg_lower.contains(*ind))
            .count();

        count >= 2
    }

    /// Get all registered skills
    pub fn list_skills(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.skill_id.as_str()).collect()
    }

    /// Get all registered workflows
    pub fn list_workflows(&self) -> Vec<&str> {
        self.workflows
            .iter()
            .map(|w| w.workflow_id.as_str())
            .collect()
    }
}

/// Global preprocessor instance
static GLOBAL_PREPROCESSOR: std::sync::OnceLock<Arc<PreProcessHook>> = std::sync::OnceLock::new();

/// Get the global preprocessor
pub fn global_preprocessor() -> Arc<PreProcessHook> {
    GLOBAL_PREPROCESSOR
        .get_or_init(|| Arc::new(PreProcessHook::new()))
        .clone()
}

/// Process message with global preprocessor
pub fn preprocess(message: &str) -> ProcessResult {
    global_preprocessor().process(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_trigger_slash_command() {
        let hook = PreProcessHook::new();

        let result = hook.process("/review this code");
        assert!(
            matches!(result, ProcessResult::SkillTriggered { skill_id, .. } if skill_id == "code-review")
        );

        let result = hook.process("/refactor the module");
        assert!(
            matches!(result, ProcessResult::SkillTriggered { skill_id, .. } if skill_id == "refactor")
        );
    }

    #[test]
    fn test_skill_trigger_chinese() {
        let hook = PreProcessHook::new();

        let result = hook.process("审查这段代码");
        assert!(
            matches!(result, ProcessResult::SkillTriggered { skill_id, .. } if skill_id == "code-review")
        );

        let result = hook.process("调试这个bug");
        assert!(
            matches!(result, ProcessResult::SkillTriggered { skill_id, .. } if skill_id == "debug")
        );
    }

    #[test]
    fn test_workflow_trigger() {
        let hook = PreProcessHook::new();

        let result = hook.process("generate article about Rust performance");
        assert!(
            matches!(result, ProcessResult::WorkflowTriggered { workflow_id, .. } if workflow_id == "image-article")
        );
    }

    #[test]
    fn test_continue_normal() {
        let hook = PreProcessHook::new();

        let result = hook.process("What is the weather today?");
        assert!(matches!(result, ProcessResult::Continue));

        let result = hook.process("Help me write a function");
        assert!(matches!(result, ProcessResult::Continue));
    }

    #[test]
    fn test_confidence_threshold() {
        let hook = PreProcessHook::new().with_confidence_threshold(0.85);

        // Should still work for high-confidence matches (0.9 > 0.85)
        let result = hook.process("/review");
        assert!(matches!(result, ProcessResult::SkillTriggered { .. }));
    }

    #[test]
    fn test_custom_skill() {
        let mut hook = PreProcessHook::new();
        hook.add_skill(SkillPattern::new(
            "custom",
            vec!["/custom", "custom skill"],
            0.9,
        ));

        let result = hook.process("/custom task");
        assert!(
            matches!(result, ProcessResult::SkillTriggered { skill_id, .. } if skill_id == "custom")
        );
    }

    #[test]
    fn test_extract_inputs() {
        let hook = PreProcessHook::new();

        let result = hook.process("generate article about Rust async programming");
        if let ProcessResult::WorkflowTriggered { inputs, .. } = result {
            assert!(inputs.contains_key("topic"));
            assert!(inputs["topic"].to_lowercase().contains("rust"));
        } else {
            panic!("Expected WorkflowTriggered");
        }
    }

    #[test]
    fn test_has_skill_intent() {
        let hook = PreProcessHook::new();

        assert!(hook.has_skill_intent("Please review my code"));
        assert!(hook.has_skill_intent("审查代码"));
        assert!(!hook.has_skill_intent("What's the time?"));
    }

    #[test]
    fn test_has_workflow_intent() {
        let hook = PreProcessHook::new();

        assert!(hook.has_workflow_intent("Analyze the code and then generate a report"));
        assert!(hook.has_workflow_intent("分析代码，然后生成报告"));
        assert!(!hook.has_workflow_intent("Just a simple question"));
    }

    #[test]
    fn test_list_skills() {
        let hook = PreProcessHook::new();
        let skills = hook.list_skills();

        assert!(skills.contains(&"code-review"));
        assert!(skills.contains(&"refactor"));
        assert!(skills.contains(&"debug"));
    }

    #[test]
    fn test_list_workflows() {
        let hook = PreProcessHook::new();
        let workflows = hook.list_workflows();

        assert!(workflows.contains(&"image-article"));
        assert!(workflows.contains(&"code-analysis"));
    }
}
