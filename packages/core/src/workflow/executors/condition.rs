//! Condition Executor
//!
//! 条件判断执行器，使用 rule_engine 进行条件判断和分支选择。

use anyhow::Result;
use async_trait::async_trait;

use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;
use crate::workflow::rule_engine::evaluate_expression;
use crate::workflow::template::TemplateRenderer;
use super::node_executor::NodeExecutor;

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