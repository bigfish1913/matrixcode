//! Workflow Create/Edit Tool
//!
//! 帮助用户创建、编辑、验证 workflow YAML 文件的工具
//!
//! 核心功能：
//! - 创建：从零开始创建 workflow
//! - 编辑：修改现有 workflow 的节点、边、参数
//! - 模板：提供预定义模板
//! - 验证：检查 workflow 结构合法性

use anyhow::{Result, bail};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::{
    EdgeDef, InputDef, NodeDef, NodeType, OutputDef,
    parser::{parse_workflow, parse_workflow_from_file, to_yaml},
};
use std::fs;
use std::path::PathBuf;

/// 获取 workflow 保存目录
fn get_workflow_dir(location: &str) -> Result<PathBuf> {
    match location {
        "project" => {
            let cwd = std::env::current_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;
            Ok(cwd.join(".matrix").join("workflows"))
        }
        "user" => dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))
            .map(|p| p.join(".matrix").join("workflows")),
        _ => bail!("Invalid location: {}. Use 'project' or 'user'", location),
    }
}

/// Workflow 创建工具
pub struct WorkflowCreateTool;

#[async_trait]
impl Tool for WorkflowCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_create".to_string(),
            description: "创建和编辑 workflow YAML 文件，支持迭代修改。

【核心功能】
此工具是 workflow 制作的核心工具，支持完整的创建-编辑-验证循环。

【节点类型和必需字段】
**基础字段**（所有节点）:
- id: 节点唯一标识
- type: 节点类型 (start/end/task/condition/parallel/approval/wait/subworkflow)
- name: 节点显示名称

**特殊字段**（按节点类型）:
- **task**: task（任务名）, params（参数）, timeout_ms, on_failure
- **condition**: branches（条件分支列表，**必须用 name 字段**）
  - 每个分支: {\"name\": \"分支名\", \"condition\": \"表达式\", \"target\": \"目标节点ID\"}
  - ⚠️ 分支用 `name` 而非 `id`
- **approval**: approvers（审批人列表）, timeout_ms
- **parallel/pipeline**: parallel_branches（并行分支列表）
  - 每个分支: {\"name\": \"分支名\", \"nodes\": [节点列表]}
- **wait**: wait_ms（等待时间）
- **subworkflow**: workflow（子工作流名称）

【功能模式】
- **create**: 创建新 workflow（首次创建）
- **edit**: 编辑现有 workflow（精确修改节点、边、参数）
- **template**: 获取模板（快速开始，推荐先用模板）
- **validate**: 验证结构（检查错误）
- **info**: 查看 workflow 详情

【推荐流程】
1. template → 获取 condition/parallel/approval 模板作为起点
2. create → 创建初始版本
3. validate → 检查是否有错误
4. edit → 多次调整直到满意

【常见错误】
1. ❌ branches 分支用 id 而非 name → 正确: {\"name\": \"分支名\", ...}
2. ❌ 缺少 start 或 end 节点 → 必须有这两种节点
3. ❌ 边引用不存在节点 → 检查节点 ID 是否正确
4. ❌ approval 缺少 approvers → 必须指定审批人

【使用示例】
获取 condition 模板:
{\"mode\": \"template\", \"template_type\": \"condition\"}

创建带条件分支的节点:
{\"id\": \"check\", \"type\": \"condition\", \"name\": \"检查\", 
 \"branches\": [{\"name\": \"是\", \"condition\": \"{{value}} > 0\", \"target\": \"yes_node\"}]}

创建审批节点:
{\"id\": \"approve\", \"type\": \"approval\", \"name\": \"用户确认\",
 \"approvers\": [\"user\"], \"timeout_ms\": 300000}".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["create", "edit", "template", "validate", "info"],
                        "description": "操作模式",
                        "default": "create"
                    },
                    "workflow_id": {
                        "type": "string",
                        "description": "目标 workflow ID (edit/info 模式必需)"
                    },
                    "workflow": {
                        "type": "object",
                        "description": "完整 workflow 定义 (create 模式)"
                    },
                    "yaml_content": {
                        "type": "string",
                        "description": "YAML 内容字符串 (可选)"
                    },
                    "edit_operation": {
                        "type": "string",
                        "enum": [
                            "add_node", "remove_node", "update_node",
                            "add_edge", "remove_edge",
                            "add_input", "remove_input", "update_input",
                            "add_output", "remove_output", "update_output",
                            "update_metadata"
                        ],
                        "description": "编辑操作类型 (edit 模式)"
                    },
                    "edit_target": {
                        "type": "string",
                        "description": "编辑目标：节点ID、边ID、输入参数名等"
                    },
                    "edit_value": {
                        "type": "object",
                        "description": "新值：节点定义、边定义、参数定义等"
                    },
                    "location": {
                        "type": "string",
                        "enum": ["project", "user"],
                        "description": "保存位置",
                        "default": "project"
                    },
                    "template_type": {
                        "type": "string",
                        "enum": ["simple", "parallel", "condition", "research", "batch", "approval"],
                        "description": "模板类型 (template 模式)"
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "是否覆盖已存在文件",
                        "default": false
                    }
                },
                "required": ["mode"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let mode = params["mode"].as_str().unwrap_or("create");

        match mode {
            "create" => self.handle_create(params).await,
            "edit" => self.handle_edit(params).await,
            "template" => self.handle_template(params).await,
            "validate" => self.handle_validate(params).await,
            "info" => self.handle_info(params).await,
            _ => bail!(
                "Invalid mode: {}. Supported modes: create, edit, template, validate, info",
                mode
            ),
        }
    }
}

impl WorkflowCreateTool {
    /// 处理创建 workflow
    async fn handle_create(&self, params: Value) -> Result<String> {
        // 获取 workflow 定义
        let workflow_def = if let Some(yaml_str) = params["yaml_content"].as_str() {
            // 从 YAML 解析
            parse_workflow(yaml_str).map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
        } else if let Some(workflow_json) = params.get("workflow") {
            // 从 JSON 解析
            serde_json::from_value(workflow_json.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse workflow JSON: {}", e))?
        } else {
            bail!(
                "Missing workflow definition. Provide either 'workflow' (JSON) or 'yaml_content' (YAML string)"
            );
        };

        // 验证 workflow
        workflow_def
            .validate()
            .map_err(|e| anyhow::anyhow!("Workflow validation failed: {}", e))?;

        // 获取保存位置
        let location = params["location"].as_str().unwrap_or("project");
        let overwrite = params["overwrite"].as_bool().unwrap_or(false);

        let save_dir = get_workflow_dir(location)?;

        // 创建目录（如果不存在）
        fs::create_dir_all(&save_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create directory {:?}: {}", save_dir, e))?;

        // 检查文件是否存在
        let file_path = save_dir.join(format!("{}.yaml", workflow_def.id));
        if file_path.exists() && !overwrite {
            bail!(
                "Workflow file already exists: {:?}\nSet overwrite=true to replace it.",
                file_path
            );
        }

        // 转换为 YAML
        let yaml_content = to_yaml(&workflow_def)
            .map_err(|e| anyhow::anyhow!("Failed to convert to YAML: {}", e))?;

        // 保存文件
        fs::write(&file_path, &yaml_content)
            .map_err(|e| anyhow::anyhow!("Failed to write file {:?}: {}", file_path, e))?;

        Ok(format!(
            "✓ Workflow created successfully!\n\n📄 File: {:?}\n📁 ID: {}\n📝 Name: {}\n\n```yaml\n{}\n```\n\nUse 'workflow_run' to execute this workflow.",
            file_path, workflow_def.id, workflow_def.name, yaml_content
        ))
    }

    /// 处理模板请求
    async fn handle_template(&self, params: Value) -> Result<String> {
        let template_type = params["template_type"].as_str().unwrap_or("simple");

        let template = match template_type {
            "simple" => self.get_simple_template(),
            "parallel" => self.get_parallel_template(),
            "condition" => self.get_condition_template(),
            "research" => self.get_research_template(),
            "batch" => self.get_batch_template(),
            "approval" => self.get_approval_template(),
            _ => bail!(
                "Unknown template type: {}. Available: simple, parallel, condition, research, batch, approval",
                template_type
            ),
        };

        Ok(format!(
            "📋 Workflow Template: {}\n\n```yaml\n{}\n```\n\n💡 Tips:\n- Copy this template and modify as needed\n- Use 'workflow_create' with mode='create' to save it\n- Each workflow must have a start node and an end node",
            template_type, template
        ))
    }

    /// 验证 workflow 结构
    async fn handle_validate(&self, params: Value) -> Result<String> {
        let workflow_def = if let Some(yaml_str) = params["yaml_content"].as_str() {
            parse_workflow(yaml_str).map_err(|e| anyhow::anyhow!("YAML parsing failed: {}", e))?
        } else if let Some(workflow_json) = params.get("workflow") {
            serde_json::from_value(workflow_json.clone())
                .map_err(|e| anyhow::anyhow!("JSON parsing failed: {}", e))?
        } else {
            bail!(
                "Missing workflow definition. Provide either 'workflow' (JSON) or 'yaml_content' (YAML string)"
            );
        };

        // 执行验证
        match workflow_def.validate() {
            Ok(()) => Ok(format!(
                "✓ Workflow validation passed!\n\n📁 ID: {}\n📝 Name: {}\n📊 Nodes: {}\n🔗 Edges: {}\n📥 Inputs: {}\n📤 Outputs: {}",
                workflow_def.id,
                workflow_def.name,
                workflow_def.nodes.len(),
                workflow_def.edges.len(),
                workflow_def.inputs.len(),
                workflow_def.outputs.len()
            )),
            Err(e) => bail!("Workflow validation failed: {}", e),
        }
    }

    /// 处理编辑 workflow
    async fn handle_edit(&self, params: Value) -> Result<String> {
        // 获取 workflow ID
        let workflow_id = params["workflow_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_id' parameter"))?;

        // 获取保存位置
        let location = params["location"].as_str().unwrap_or("project");
        let save_dir = get_workflow_dir(location)?;
        let file_path = save_dir.join(format!("{}.yaml", workflow_id));

        // 直接从文件加载 workflow
        if !file_path.exists() {
            bail!("Workflow '{}' not found at {:?}", workflow_id, file_path);
        }

        let mut workflow = parse_workflow_from_file(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to load workflow: {}", e))?;

        // 获取编辑操作类型
        let edit_operation = params["edit_operation"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'edit_operation' parameter"))?;

        let edit_target = params["edit_target"].as_str();
        let edit_value = params.get("edit_value").cloned();

        // 执行编辑操作
        match edit_operation {
            // 节点操作
            "add_node" => {
                let node_json = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for add_node"))?;
                let new_node: NodeDef = serde_json::from_value(node_json)
                    .map_err(|e| anyhow::anyhow!("Invalid node definition: {}", e))?;

                // 检查节点ID是否已存在
                if workflow.nodes.iter().any(|n| n.id == new_node.id) {
                    bail!("Node '{}' already exists", new_node.id);
                }

                workflow.nodes.push(new_node);
            }

            "remove_node" => {
                let node_id = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for remove_node"))?;
                let idx = workflow
                    .nodes
                    .iter()
                    .position(|n| n.id == node_id)
                    .ok_or_else(|| anyhow::anyhow!("Node '{}' not found", node_id))?;

                // 不能删除 start/end 节点
                let node = &workflow.nodes[idx];
                if node.node_type == NodeType::Start || node.node_type == NodeType::End {
                    bail!("Cannot remove start/end nodes");
                }

                // 删除相关边
                workflow
                    .edges
                    .retain(|e| e.from != node_id && e.to != node_id);
                workflow.nodes.remove(idx);
            }

            "update_node" => {
                let node_id = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for update_node"))?;
                let node = workflow
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == node_id)
                    .ok_or_else(|| anyhow::anyhow!("Node '{}' not found", node_id))?;

                let updates = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for update_node"))?;

                // 更新节点属性
                if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
                    node.name = name.to_string();
                }
                if let Some(task) = updates.get("task").and_then(|v| v.as_str()) {
                    node.task = Some(task.to_string());
                }
                if let Some(params_val) = updates.get("params") {
                    node.params = serde_json::from_value(params_val.clone())
                        .map_err(|e| anyhow::anyhow!("Invalid params: {}", e))?;
                }
                if let Some(timeout) = updates.get("timeout_ms").and_then(|v| v.as_u64()) {
                    node.timeout_ms = Some(timeout);
                }
            }

            // 边操作
            "add_edge" => {
                let edge_json = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for add_edge"))?;
                let new_edge: EdgeDef = serde_json::from_value(edge_json)
                    .map_err(|e| anyhow::anyhow!("Invalid edge definition: {}", e))?;

                // 检查源和目标节点是否存在
                if !workflow.nodes.iter().any(|n| n.id == new_edge.from) {
                    bail!("Source node '{}' not found", new_edge.from);
                }
                if !workflow.nodes.iter().any(|n| n.id == new_edge.to) {
                    bail!("Target node '{}' not found", new_edge.to);
                }

                workflow.edges.push(new_edge);
            }

            "remove_edge" => {
                let from = edit_target.ok_or_else(|| {
                    anyhow::anyhow!("Missing 'edit_target' (from node) for remove_edge")
                })?;
                let to = params["to"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'to' parameter for remove_edge"))?;

                workflow.edges.retain(|e| e.from != from || e.to != to);
            }

            // 输入参数操作
            "add_input" => {
                let input_json = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for add_input"))?;
                let new_input: InputDef = serde_json::from_value(input_json)
                    .map_err(|e| anyhow::anyhow!("Invalid input definition: {}", e))?;

                workflow.inputs.push(new_input);
            }

            "remove_input" => {
                let input_name = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for remove_input"))?;
                workflow.inputs.retain(|i| i.name != input_name);
            }

            "update_input" => {
                let input_name = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for update_input"))?;
                let input = workflow
                    .inputs
                    .iter_mut()
                    .find(|i| i.name == input_name)
                    .ok_or_else(|| anyhow::anyhow!("Input '{}' not found", input_name))?;

                let updates = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for update_input"))?;

                if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
                    input.description = Some(desc.to_string());
                }
                if let Some(required) = updates.get("required").and_then(|v| v.as_bool()) {
                    input.required = required;
                }
                if let Some(default) = updates.get("default") {
                    input.default = Some(default.clone());
                }
            }

            // 输出参数操作
            "add_output" => {
                let output_json = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for add_output"))?;
                let new_output: OutputDef = serde_json::from_value(output_json)
                    .map_err(|e| anyhow::anyhow!("Invalid output definition: {}", e))?;

                workflow.outputs.push(new_output);
            }

            "remove_output" => {
                let output_name = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for remove_output"))?;
                workflow.outputs.retain(|o| o.name != output_name);
            }

            "update_output" => {
                let output_name = edit_target
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_target' for update_output"))?;
                let output = workflow
                    .outputs
                    .iter_mut()
                    .find(|o| o.name == output_name)
                    .ok_or_else(|| anyhow::anyhow!("Output '{}' not found", output_name))?;

                let updates = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for update_output"))?;

                if let Some(value) = updates.get("value").and_then(|v| v.as_str()) {
                    output.value = value.to_string();
                }
                if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
                    output.description = Some(desc.to_string());
                }
            }

            // 元数据操作
            "update_metadata" => {
                let updates = edit_value
                    .ok_or_else(|| anyhow::anyhow!("Missing 'edit_value' for update_metadata"))?;

                if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
                    workflow.name = name.to_string();
                }
                if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
                    workflow.description = Some(desc.to_string());
                }
                if let Some(version) = updates.get("version").and_then(|v| v.as_str()) {
                    workflow.version = version.to_string();
                }
            }

            _ => bail!("Unknown edit operation: {}", edit_operation),
        }

        // 验证修改后的 workflow
        workflow
            .validate()
            .map_err(|e| anyhow::anyhow!("Workflow validation failed after edit: {}", e))?;

        // 保存文件
        let yaml_content =
            to_yaml(&workflow).map_err(|e| anyhow::anyhow!("Failed to convert to YAML: {}", e))?;
        fs::write(&file_path, &yaml_content)
            .map_err(|e| anyhow::anyhow!("Failed to write file {:?}: {}", file_path, e))?;

        Ok(format!(
            "✓ Workflow '{}' updated successfully!\n\nOperation: {}\n📄 File: {:?}\n\n```yaml\n{}\n```\n\nUse 'info' mode to see detailed structure.",
            workflow_id, edit_operation, file_path, yaml_content
        ))
    }

    /// 处理查看 workflow 信息
    async fn handle_info(&self, params: Value) -> Result<String> {
        let workflow_id = params["workflow_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_id' parameter"))?;

        // 获取保存位置
        let location = params["location"].as_str().unwrap_or("project");
        let save_dir = get_workflow_dir(location)?;
        let file_path = save_dir.join(format!("{}.yaml", workflow_id));

        // 直接从文件加载
        if !file_path.exists() {
            bail!("Workflow '{}' not found at {:?}", workflow_id, file_path);
        }

        let workflow = parse_workflow_from_file(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to load workflow: {}", e))?;

        let mut info = format!(
            "📋 Workflow: {}\n\n📁 ID: {}\n📝 Name: {}\n📌 Version: {}\n",
            workflow_id, workflow.id, workflow.name, workflow.version
        );

        if let Some(ref desc) = workflow.description {
            info.push_str(&format!("📖 Description: {}\n", desc));
        }

        // 节点信息
        info.push_str("\n📊 Nodes:\n");
        for node in &workflow.nodes {
            let node_type = match node.node_type {
                NodeType::Start => "start",
                NodeType::End => "end",
                NodeType::Task => "task",
                NodeType::Condition => "condition",
                NodeType::Parallel => "parallel",
                NodeType::Pipeline => "pipeline",
                NodeType::Wait => "wait",
                NodeType::Approval => "approval",
                NodeType::SubWorkflow => "subworkflow",
            };
            info.push_str(&format!("  - {} [{}] {}\n", node.id, node_type, node.name));

            if let Some(ref task) = node.task {
                info.push_str(&format!("    Task: {}\n", task));
            }
        }

        // 边信息
        info.push_str("\n🔗 Edges:\n");
        for edge in &workflow.edges {
            let label = edge
                .label
                .as_ref()
                .map(|l| format!(" ({})", l))
                .unwrap_or_default();
            info.push_str(&format!("  {} → {}{}\n", edge.from, edge.to, label));
        }

        // 输入参数
        if !workflow.inputs.is_empty() {
            info.push_str("\n📥 Inputs:\n");
            for input in &workflow.inputs {
                let required = if input.required { " (required)" } else { "" };
                info.push_str(&format!(
                    "  - {} [{}]{}\n",
                    input.name, input.input_type, required
                ));
                if let Some(ref desc) = input.description {
                    info.push_str(&format!("    {}\n", desc));
                }
            }
        }

        // 输出参数
        if !workflow.outputs.is_empty() {
            info.push_str("\n📤 Outputs:\n");
            for output in &workflow.outputs {
                info.push_str(&format!("  - {}: {}\n", output.name, output.value));
            }
        }

        Ok(info)
    }

    /// 简单工作流模板
    fn get_simple_template(&self) -> String {
        r#"id: my-workflow
name: My Workflow
version: "1.0.0"
description: A simple workflow example using bash tool

inputs:
  - name: param1
    type: string
    required: true
    description: First parameter

outputs:
  - name: result
    value: "{{nodes.task1.output}}"

nodes:
  - id: start
    type: start
    name: Start

  - id: task1
    type: task
    name: First Task
    task: bash
    params:
      command: "echo '{{inputs.param1}}'"
    timeout_ms: 5000
    on_failure:
      type: abort

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: task1
  - from: task1
    to: end
"#
        .to_string()
    }

    /// 并行工作流模板
    fn get_parallel_template(&self) -> String {
        r#"id: parallel-workflow
name: Parallel Workflow
version: "1.0.0"
description: A workflow with parallel execution using bash tool

inputs:
  - name: data
    type: string
    required: true
    description: Input data to process

nodes:
  - id: start
    type: start
    name: Start

  - id: parallel1
    type: parallel
    name: Parallel Processing
    parallel_branches:
      - name: Branch A
        nodes:
          - id: task_a
            type: task
            name: Task A
            task: bash
            params:
              command: "echo 'Processing A: {{inputs.data}}'"
      - name: Branch B
        nodes:
          - id: task_b
            type: task
            name: Task B
            task: bash
            params:
              command: "echo 'Processing B: {{inputs.data}}'"

  - id: merge
    type: task
    name: Merge Results
    task: bash
    params:
      command: "echo 'Merged results from A and B'"
    timeout_ms: 10000

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: parallel1
  - from: parallel1
    to: merge
  - from: merge
    to: end
"#
        .to_string()
    }

    /// 条件工作流模板
    fn get_condition_template(&self) -> String {
        r#"id: condition-workflow
name: Conditional Workflow
version: "1.0.0"
description: A workflow with conditional branches using bash tool

inputs:
  - name: value
    type: number
    required: true
    description: Value to check

nodes:
  - id: start
    type: start
    name: Start

  - id: check_value
    type: condition
    name: Check Value
    branches:
      - name: Positive
        condition: "{{inputs.value}} > 0"
        target: positive_task
      - name: Negative
        condition: "{{inputs.value}} < 0"
        target: negative_task
      - name: Zero
        condition: "{{inputs.value}} == 0"
        target: zero_task

  - id: positive_task
    type: task
    name: Handle Positive
    task: bash
    params:
      command: "echo 'Value is positive'"
    timeout_ms: 5000

  - id: negative_task
    type: task
    name: Handle Negative
    task: bash
    params:
      command: "echo 'Value is negative'"
    timeout_ms: 5000

  - id: zero_task
    type: task
    name: Handle Zero
    task: bash
    params:
      command: "echo 'Value is zero'"
    timeout_ms: 5000

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: check_value
  - from: positive_task
    to: end
  - from: negative_task
    to: end
  - from: zero_task
    to: end
"#
        .to_string()
    }

    /// 研究工作流模板
    fn get_research_template(&self) -> String {
        r#"id: research-workflow
name: Research Workflow
version: "1.0.0"
description: Automated research and report generation

inputs:
  - name: topic
    type: string
    required: true
    description: Research topic
  - name: depth
    type: string
    required: false
    default: "medium"
    description: Research depth (shallow/medium/deep)

outputs:
  - name: report
    value: "{{nodes.generate_report.output}}"

nodes:
  - id: start
    type: start
    name: Start

  - id: search_web
    type: task
    name: Search Web
    task: websearch
    params:
      query: "{{inputs.topic}}"
      max_results: 10
    on_failure:
      type: retry
      max_attempts: 3

  - id: analyze_results
    type: task
    name: Analyze Results
    task: analyze
    params:
      data: "{{nodes.search_web.output}}"
      depth: "{{inputs.depth}}"

  - id: generate_report
    type: task
    name: Generate Report
    task: content_generation
    params:
      topic: "{{inputs.topic}}"
      research_data: "{{nodes.analyze_results.output}}"
      style: "professional"

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: search_web
  - from: search_web
    to: analyze_results
  - from: analyze_results
    to: generate_report
  - from: generate_report
    to: end
"#
        .to_string()
    }

    /// 批量操作工作流模板
    fn get_batch_template(&self) -> String {
        r#"id: batch-workflow
name: Batch Processing Workflow
version: "1.0.0"
description: Process multiple items in batch

inputs:
  - name: items
    type: array
    required: true
    description: Array of items to process
  - name: operation
    type: string
    required: true
    description: Operation to perform on each item

nodes:
  - id: start
    type: start
    name: Start

  - id: prepare_batch
    type: task
    name: Prepare Batch
    task: prepare_batch
    params:
      items: "{{inputs.items}}"

  - id: process_items
    type: parallel
    name: Process Items
    parallel_branches:
      - name: Process Chunk 1
        nodes:
          - id: chunk1
            type: task
            name: Process Chunk 1
            task: "{{inputs.operation}}"
            params:
              items: "{{nodes.prepare_batch.chunk1}}"
      - name: Process Chunk 2
        nodes:
          - id: chunk2
            type: task
            name: Process Chunk 2
            task: "{{inputs.operation}}"
            params:
              items: "{{nodes.prepare_batch.chunk2}}"

  - id: aggregate_results
    type: task
    name: Aggregate Results
    task: aggregate
    params:
      results:
        - "{{nodes.chunk1.output}}"
        - "{{nodes.chunk2.output}}"

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: prepare_batch
  - from: prepare_batch
    to: process_items
  - from: process_items
    to: aggregate_results
  - from: aggregate_results
    to: end
"#
        .to_string()
    }

    /// 审批工作流模板
    fn get_approval_template(&self) -> String {
        r#"id: approval-workflow
name: Approval Workflow
version: "1.0.0"
description: Workflow with user approval checkpoint

inputs:
  - name: task_data
    type: string
    required: true
    description: Data to process before approval

outputs:
  - name: final_result
    value: "{{nodes.final_task.output}}"

nodes:
  - id: start
    type: start
    name: Start

  - id: prepare_task
    type: task
    name: Prepare Content
    task: bash
    params:
      command: "echo 'Processing: {{inputs.task_data}}'"
    timeout_ms: 10000

  - id: user_approval
    type: approval
    name: User Approval
    description: Please review and approve the result
    approvers:
      - "user"
    timeout_ms: 300000

  - id: check_approval
    type: condition
    name: Check Approval Result
    branches:
      - name: Approved
        condition: "{{approval_result}} == 'approved'"
        target: final_task
      - name: Rejected
        condition: "{{approval_result}} == 'rejected'"
        target: revise_task

  - id: revise_task
    type: task
    name: Revise Content
    task: bash
    params:
      command: "echo 'Revising content based on feedback'"
    timeout_ms: 10000

  - id: final_task
    type: task
    name: Final Output
    task: bash
    params:
      command: "echo 'Final approved output'"
    timeout_ms: 10000

  - id: end
    type: end
    name: End

edges:
  - from: start
    to: prepare_task
  - from: prepare_task
    to: user_approval
  - from: user_approval
    to: check_approval
  - from: check_approval
    to: final_task
    label: Approved
  - from: check_approval
    to: revise_task
    label: Rejected
  - from: revise_task
    to: user_approval
  - from: final_task
    to: end
"#
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_template_simple() {
        let tool = WorkflowCreateTool;
        let result = tool
            .execute(json!({
                "mode": "template",
                "template_type": "simple"
            }))
            .await
            .unwrap();

        assert!(result.contains("simple"));
        assert!(result.contains("id:"));
        assert!(result.contains("nodes:"));
        assert!(result.contains("edges:"));
    }

    #[tokio::test]
    async fn test_template_research() {
        let tool = WorkflowCreateTool;
        let result = tool
            .execute(json!({
                "mode": "template",
                "template_type": "research"
            }))
            .await
            .unwrap();

        assert!(result.contains("research"));
        assert!(result.contains("websearch"));
        assert!(result.contains("generate_report"));
    }

    #[tokio::test]
    async fn test_validate_valid_workflow() {
        let tool = WorkflowCreateTool;
        let result = tool.execute(json!({
            "mode": "validate",
            "yaml_content": "id: test\nname: Test\nnodes:\n  - id: start\n    type: start\n    name: Start\n  - id: end\n    type: end\n    name: End\nedges:\n  - from: start\n    to: end"
        })).await.unwrap();

        assert!(result.contains("validation passed"));
        assert!(result.contains("Nodes: 2"));
        assert!(result.contains("Edges: 1"));
    }

    #[tokio::test]
    async fn test_validate_invalid_workflow() {
        let tool = WorkflowCreateTool;
        let result = tool.execute(json!({
            "mode": "validate",
            "yaml_content": "id: test\nname: Test\nnodes:\n  - id: task1\n    type: task\n    name: Task"
        })).await;

        // Should fail because no start/end nodes
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("validation failed")
        );
    }

    #[tokio::test]
    async fn test_info_mode() {
        let tool = WorkflowCreateTool;

        // 先创建一个 workflow
        let create_result = tool.execute(json!({
            "mode": "create",
            "workflow": {
                "id": "test-info-workflow",
                "name": "Test Info Workflow",
                "version": "1.0.0",
                "description": "A test workflow for info mode",
                "nodes": [
                    {"id": "start", "type": "start", "name": "Start"},
                    {"id": "task1", "type": "task", "name": "Task 1", "task": "process"},
                    {"id": "end", "type": "end", "name": "End"}
                ],
                "edges": [
                    {"from": "start", "to": "task1"},
                    {"from": "task1", "to": "end"}
                ],
                "inputs": [
                    {"name": "data", "type": "string", "required": true, "description": "Input data"}
                ],
                "outputs": [
                    {"name": "result", "value": "{{nodes.task1.output}}"}
                ]
            },
            "location": "project",
            "overwrite": true
        })).await.unwrap();

        assert!(create_result.contains("created successfully"));

        // 测试 info 模式
        let info_result = tool
            .execute(json!({
                "mode": "info",
                "workflow_id": "test-info-workflow"
            }))
            .await
            .unwrap();

        assert!(info_result.contains("test-info-workflow"));
        assert!(info_result.contains("Test Info Workflow"));
        assert!(info_result.contains("Task 1"));
        assert!(info_result.contains("start → task1"));
        assert!(info_result.contains("data [string] (required)"));
    }

    #[tokio::test]
    async fn test_edit_update_node() {
        let tool = WorkflowCreateTool;

        // 先创建一个 workflow
        tool.execute(json!({
            "mode": "create",
            "workflow": {
                "id": "test-edit-workflow",
                "name": "Test Edit Workflow",
                "version": "1.0.0",
                "nodes": [
                    {"id": "start", "type": "start", "name": "Start"},
                    {"id": "task1", "type": "task", "name": "Old Name", "task": "old_task"},
                    {"id": "end", "type": "end", "name": "End"}
                ],
                "edges": [
                    {"from": "start", "to": "task1"},
                    {"from": "task1", "to": "end"}
                ]
            },
            "location": "project",
            "overwrite": true
        }))
        .await
        .unwrap();

        // 测试 update_node
        let edit_result = tool
            .execute(json!({
                "mode": "edit",
                "workflow_id": "test-edit-workflow",
                "edit_operation": "update_node",
                "edit_target": "task1",
                "edit_value": {
                    "name": "New Name",
                    "task": "new_task"
                }
            }))
            .await
            .unwrap();

        assert!(edit_result.contains("updated successfully"));
        assert!(edit_result.contains("update_node"));

        // 验证更新
        let info_result = tool
            .execute(json!({
                "mode": "info",
                "workflow_id": "test-edit-workflow"
            }))
            .await
            .unwrap();

        assert!(info_result.contains("New Name"));
        assert!(info_result.contains("new_task"));
        assert!(!info_result.contains("Old Name"));
    }

    #[tokio::test]
    async fn test_edit_add_node() {
        let tool = WorkflowCreateTool;

        // 先创建一个 workflow
        tool.execute(json!({
            "mode": "create",
            "workflow": {
                "id": "test-add-node-workflow",
                "name": "Test Add Node",
                "version": "1.0.0",
                "nodes": [
                    {"id": "start", "type": "start", "name": "Start"},
                    {"id": "end", "type": "end", "name": "End"}
                ],
                "edges": [
                    {"from": "start", "to": "end"}
                ]
            },
            "location": "project",
            "overwrite": true
        }))
        .await
        .unwrap();

        // 测试 add_node
        let edit_result = tool
            .execute(json!({
                "mode": "edit",
                "workflow_id": "test-add-node-workflow",
                "edit_operation": "add_node",
                "edit_value": {
                    "id": "new_task",
                    "type": "task",
                    "name": "New Task",
                    "task": "process"
                }
            }))
            .await
            .unwrap();

        assert!(edit_result.contains("updated successfully"));

        // 测试 add_edge
        let edge_result = tool
            .execute(json!({
                "mode": "edit",
                "workflow_id": "test-add-node-workflow",
                "edit_operation": "add_edge",
                "edit_value": {
                    "from": "start",
                    "to": "new_task"
                }
            }))
            .await
            .unwrap();

        assert!(edge_result.contains("updated successfully"));

        // 再添加一条边
        tool.execute(json!({
            "mode": "edit",
            "workflow_id": "test-add-node-workflow",
            "edit_operation": "add_edge",
            "edit_value": {
                "from": "new_task",
                "to": "end"
            }
        }))
        .await
        .unwrap();

        // 验证节点和边
        let info_result = tool
            .execute(json!({
                "mode": "info",
                "workflow_id": "test-add-node-workflow"
            }))
            .await
            .unwrap();

        assert!(info_result.contains("new_task"));
        assert!(info_result.contains("New Task"));
        assert!(info_result.contains("start → new_task"));
        assert!(info_result.contains("new_task → end"));
    }

    #[tokio::test]
    async fn test_invalid_mode() {
        let tool = WorkflowCreateTool;
        let result = tool
            .execute(json!({
                "mode": "invalid"
            }))
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_definition() {
        let tool = WorkflowCreateTool;
        let def = tool.definition();

        assert_eq!(def.name, "workflow_create");
        assert!(def.description.contains("创建")); // Description contains "创建"
        assert!(def.parameters["properties"]["mode"].is_object());
    }
}
