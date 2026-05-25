//! Workflow Tool for Agent
//!
//! Provides tool interface for AI to execute workflows

use matrixcode_core::tools::{Tool, ToolDefinition, ToolParameter};
use matrixcode_core::workflow::{WorkflowRegistry, WorkflowEngine, WorkflowPersistence, WorkflowStatus};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Tool to discover available workflows
pub struct WorkflowDiscoverTool;

#[async_trait]
impl Tool for WorkflowDiscoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_discover",
            description: "Discover available workflows. Returns a list of workflow IDs and descriptions that can be executed.",
            parameters: vec![],
        }
    }

    async fn execute(&self, _params: HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        if registry.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "message": "No workflows found. Create YAML files in .matrix/workflows/ or ~/.matrix/workflows/",
                "workflows": []
            }));
        }

        let workflows: Vec<serde_json::Value> = registry.list()
            .iter()
            .map(|info| serde_json::json!({
                "id": info.id,
                "name": info.name,
                "description": info.description,
                "required_inputs": info.required_inputs,
                "source": if info.source == matrixcode_core::workflow::WorkflowSource::Project { "project" } else { "global" }
            }))
            .collect();

        Ok(serde_json::json!({
            "success": true,
            "message": format!("Found {} workflows", workflows.len()),
            "workflows": workflows
        }))
    }
}

/// Tool to run a workflow
pub struct WorkflowRunTool;

#[async_trait]
impl Tool for WorkflowRunTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_run",
            description: "Execute a workflow by ID. The workflow will run through all its nodes and return the results.",
            parameters: vec![
                ToolParameter {
                    name: "workflow_id",
                    description: "The workflow ID to execute. Use workflow_discover to find available IDs.",
                    required: true,
                    schema: None,
                },
                ToolParameter {
                    name: "inputs",
                    description: "JSON object with workflow inputs. Keys must match workflow's required_inputs.",
                    required: false,
                    schema: None,
                },
            ],
        }
    }

    async fn execute(&self, params: HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
        let workflow_id = params.get("workflow_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing workflow_id parameter"))?;

        let inputs: HashMap<String, serde_json::Value> = params.get("inputs")
            .and_then(|v| {
                if v.is_object() {
                    Some(v.as_object().unwrap().clone())
                } else {
                    None
                }
            })
            .map(|m| m.into_iter().collect())
            .unwrap_or_default();

        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        // Load workflow
        let workflow_def = registry.load_workflow(workflow_id)?
            .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found. Use workflow_discover to list available workflows.", workflow_id))?;

        // Create engine and run
        let engine = WorkflowEngine::new(workflow_def)?;
        let context = engine.run(inputs).await?;

        // Save context
        let persistence = WorkflowPersistence::new(project_path.as_ref());
        if let Err(e) = persistence.save(&context) {
            log::warn!("Failed to save workflow context: {}", e);
        }

        // Build result
        let status_str = match context.status {
            WorkflowStatus::Completed => "completed",
            WorkflowStatus::Failed => "failed",
            WorkflowStatus::Paused => "paused",
            WorkflowStatus::Running => "running",
            WorkflowStatus::Cancelled => "cancelled",
            WorkflowStatus::Pending => "pending",
        };

        let execution_path: Vec<serde_json::Value> = context.execution_path.iter()
            .map(|node_id| {
                let exec = context.get_node_execution(node_id);
                serde_json::json!({
                    "node_id": node_id,
                    "status": exec.map(|e| format!("{:?}", e.status)).unwrap_or("unknown"),
                    "output": exec.and_then(|e| e.output.clone())
                })
            })
            .collect();

        Ok(serde_json::json!({
            "success": context.status == WorkflowStatus::Completed,
            "instance_id": context.instance_id,
            "workflow_id": context.workflow_id,
            "status": status_str,
            "nodes_executed": context.execution_path.len(),
            "execution_path": execution_path,
            "error": context.error,
            "variables": context.variables
        }))
    }
}

/// Tool to match workflows by intent
pub struct WorkflowMatchTool;

#[async_trait]
impl Tool for WorkflowMatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_match",
            description: "Find workflows matching a query/intent. Returns ranked list of relevant workflows.",
            parameters: vec![
                ToolParameter {
                    name: "query",
                    description: "Natural language query describing what you want to do. Examples: 'process text', 'generate code', 'validate output'",
                    required: true,
                    schema: None,
                },
            ],
        }
    }

    async fn execute(&self, params: HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing query parameter"))?;

        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        let matches = registry.match_workflows(query);

        if matches.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "message": format!("No workflows match '{}'", query),
                "matches": []
            }));
        }

        let matched_workflows: Vec<serde_json::Value> = matches.iter()
            .take(5)
            .map(|info| serde_json::json!({
                "id": info.id,
                "name": info.name,
                "description": info.description,
                "required_inputs": info.required_inputs
            }))
            .collect();

        Ok(serde_json::json!({
            "success": true,
            "message": format!("Found {} matching workflows", matched_workflows.len()),
            "query": query,
            "matches": matched_workflows
        }))
    }
}

/// Get all workflow tools
pub fn workflow_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(WorkflowDiscoverTool),
        Box::new(WorkflowRunTool),
        Box::new(WorkflowMatchTool),
    ]
}

/// Add workflow tools to existing tools
pub fn add_workflow_tools(tools: Vec<Box<dyn Tool>>) -> Vec<Box<dyn Tool>> {
    let mut all_tools = tools;
    all_tools.extend(workflow_tools());
    all_tools
}