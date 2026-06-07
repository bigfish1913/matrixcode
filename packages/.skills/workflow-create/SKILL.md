---
name: workflow-create
description: 创建 workflow YAML 文件，交互式引导用户设计自动化流程
trigger: 用户请求创建 workflow、设计自动化流程、制作工作流模板
---

# Workflow 创建技能

这个技能帮助你创建 workflow YAML 文件，用于自动化任务流程。

## 🎯 何时使用此技能

- 用户说："创建一个 workflow"
- 用户说："制作自动化流程"
- 用户说："帮我设计工作流模板"
- 用户提到多步骤自动化任务
- 用户需要批处理、研究、内容生成等自动化场景

## 📋 交互式流程

### 步骤 1：理解需求

在创建 workflow 前，先询问用户：

**必须确认的信息**：
1. Workflow 目的（要自动化什么任务？）
2. 输入参数（需要用户提供什么？）
3. 输出结果（期望得到什么？）
4. 执行流程（具体步骤是什么？）

**可选确认的信息**：
- 失败处理策略（重试？终止？跳过？）
- 并行需求（某些步骤能否并行执行？）
- 条件分支（是否需要根据条件选择不同路径？）

### 步骤 2：设计节点结构

根据需求设计 workflow 节点：

**节点类型及特殊字段**：
- `start`: 开始节点（必需，无特殊字段）
- `end`: 结束节点（必需，无特殊字段）
- `task`: 任务节点（需要 `task` 字段指定任务名，可选 `params`, `timeout_ms`, `on_failure`）
- `condition`: 条件分支节点（需要 `branches` 字段）
  - ⚠️ **重要**：branches 分支用 `name` 而非 `id`
  - 格式：`{"name": "分支名", "condition": "表达式", "target": "目标节点ID"}`
- `parallel`: 并行执行节点（需要 `parallel_branches` 字段）
  - 格式：`{"name": "分支名", "nodes": [节点列表]}`
- `approval`: 人工审批节点（需要 `approvers` 字段）
  - 格式：`{"approvers": ["user"], "timeout_ms": 300000}`
- `wait`: 等待节点（需要 `wait_ms` 字段）
- `subworkflow`: 子工作流节点（需要 `workflow` 字段指定子工作流名称）

**示例结构**：

```
简单流程：    start → task1 → task2 → end
条件分支：    start → condition → [branch_a 或 branch_b] → end
并行执行：    start → parallel → [task_a + task_b] → merge → end
审批流程：    start → task → approval → condition → [通过/拒绝] → end
```

### 步骤 3：生成 YAML

使用 `workflow_create` 工具生成并保存 YAML：

```json
{
  "mode": "create",
  "workflow": {
    "id": "unique-workflow-id",
    "name": "Workflow 名称",
    "version": "1.0.0",
    "description": "Workflow 描述",
    "inputs": [
      {
        "name": "param_name",
        "type": "string",
        "required": true,
        "description": "参数描述"
      }
    ],
    "outputs": [
      {
        "name": "result",
        "value": "{{nodes.end.output}}"
      }
    ],
    "nodes": [
      {
        "id": "start",
        "type": "start",
        "name": "开始"
      },
      {
        "id": "task1",
        "type": "task",
        "name": "任务名称",
        "task": "task_type",
        "params": {
          "input": "{{inputs.param_name}}"
        },
        "on_failure": {
          "type": "abort"
        }
      },
      {
        "id": "end",
        "type": "end",
        "name": "结束"
      }
    ],
    "edges": [
      {"from": "start", "to": "task1"},
      {"from": "task1", "to": "end"}
    ]
  }
}
```

### 步骤 4：验证和保存

验证生成的 workflow：
```json
{
  "mode": "validate",
  "yaml_content": "生成的 YAML 字符串"
}
```

保存到位置：
- `project`: `.matrix/workflows/` （项目级，团队共享）
- `user`: `~/.matrix/workflows/` （用户级，个人使用）

## 🎨 常见 Workflow 模式

### 1. 研究自动化 Workflow

适合：搜索信息、分析数据、生成报告

**特征**：
- 输入：研究主题、深度参数
- 流程：搜索 → 分析 → 生成报告
- 输出：结构化报告

**推荐模板**：
```json
{
  "mode": "template",
  "template_type": "research"
}
```

### 2. 批量处理 Workflow

适合：批量文件处理、批量数据转换

**特征**：
- 输入：数据数组、操作类型
- 流程：准备 → 并行处理 → 汇总结果
- 输出：处理结果列表

**推荐模板**：
```json
{
  "mode": "template",
  "template_type": "batch"
}
```

### 3. 内容生成 Workflow

适合：写文章、生成文档、创建内容

**特征**：
- 输入：主题、风格、素材
- 流程：准备素材 → 搜索资源 → AI 生成 → 格式化
- 输出：结构化内容文件

### 4. 条件分支 Workflow

适合：根据条件选择不同处理路径

**特征**：
- 输入：决策参数
- 流程：检查条件 → 选择分支 → 执行 → 结束
- 输出：处理结果

**推荐模板**：
```json
{
  "mode": "template",
  "template_type": "condition"
}
```

## 📚 最佳实践

### 1. 节点命名规范

- 使用清晰、描述性的名称
- ID 使用小写字母和下划线：`search_web`, `generate_report`
- Name 使用可读文本：`搜索网页`, `生成报告`

### 2. 参数传递

使用模板语法访问数据：
- 输入参数：`{{inputs.param_name}}`
- 节点输出：`{{nodes.node_id.output}}`
- 全局变量：`{{variables.var_name}}`

### 3. 失败策略

根据任务重要性选择：
- `abort`: 关键任务失败立即终止（默认）
- `retry`: 网络请求等不稳定任务，自动重试
- `ignore`: 非关键任务失败继续执行
- `goto`: 失败后跳转到特定处理节点

### 4. 超时设置

为耗时任务设置合理的超时：
```yaml
timeout_ms: 300000  # 5 分钟
```

### 5. 节点描述

为复杂节点添加描述，帮助用户理解：
```yaml
description: "搜索多个来源，汇总信息后进行分析"
```

## 🚀 完整示例

### 用户请求

> "帮我创建一个研究 workflow，自动搜索网络、分析数据、生成报告"

### AI 响应

我会帮你创建一个研究自动化 workflow。先确认几个细节：

**问题 1：研究主题**
- Workflow 会接收什么主题作为输入？
- 是否需要限定搜索范围？

**问题 2：分析深度**
- 需要 shallow（快速）、medium（适中）、deep（深度）哪种分析？
- 是否需要多个来源对比？

**问题 3：输出格式**
- 期望输出什么格式？（Markdown？JSON？HTML？）
- 报告需要包含哪些部分？

[等待用户回答后，调用 workflow_create 工具生成]

---

## 🔧 工具参考

### workflow_create 工具

**模式**：
- `create`: 创建并保存 workflow
- `template`: 获取预定义模板
- `validate`: 验证 workflow 结构

**参数**：
- `workflow`: JSON 格式的 workflow 定义
- `yaml_content`: 直接提供 YAML 字符串
- `location`: `project` 或 `user`
- `overwrite`: 是否覆盖已存在文件
- `template_type`: 模板类型（simple/parallel/condition/research/batch/approval）

**示例调用**：

获取模板：
```json
{
  "mode": "template",
  "template_type": "research"
}
```

创建 workflow：
```json
{
  "mode": "create",
  "workflow": {
    "id": "research-workflow",
    "name": "研究自动化",
    "nodes": [...],
    "edges": [...]
  },
  "location": "project",
  "overwrite": false
}
```

## ⚠️ 注意事项

1. **唯一 ID**：每个 workflow 必须有唯一的 `id` 字段
2. **必需节点**：必须有 `start` 和 `end` 节点
3. **边的完整性**：所有节点必须通过边连接（start → ... → end）
4. **输入必填项**：标记为 `required: true` 的输入必须提供 `default` 或在运行时提供
5. **文件存在检查**：不设置 `overwrite` 时，已存在文件会导致创建失败

### 常见错误

1. ❌ **branches 分支用 id 而非 name** 
   - 错误：`{"id": "分支名", ...}`
   - 正确：`{"name": "分支名", "condition": "...", "target": "..."}`

2. ❌ **approval 缺少 approvers**
   - approval 节点必须指定 `approvers` 字段

3. ❌ **边引用不存在节点**
   - 检查 edges 中的 `from` 和 `to` 是否匹配 nodes 的 `id`

4. ❌ **condition 节点缺少 branches**
   - condition 节点必须有 branches 列表

## 📖 延伸阅读

查看现有 workflow 示例：
```bash
ls ~/.matrix/workflows/*.yaml
ls .matrix/workflows/*.yaml
```

运行已创建的 workflow：
使用 `workflow_run` 工具：
```json
{
  "workflow_id": "your-workflow-id",
  "inputs": {
    "param_name": "value"
  }
}
```