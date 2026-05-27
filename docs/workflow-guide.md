# MatrixCode Workflow 使用指南

Workflow 是 MatrixCode 的自动化流程系统，通过 YAML 文件定义可复用的任务流程，由 AI 或用户直接调用执行。

## 目录

- [概述](#概述)
- [文件位置](#文件位置)
- [YAML 格式规范](#yaml-格式规范)
- [节点类型](#节点类型)
- [边与条件](#边与条件)
- [变量与模板](#变量与模板)
- [输入输出](#输入输出)
- [执行策略](#执行策略)
- [持久化](#持久化)
- [调用方式](#调用方式)
- [示例](#示例)

---

## 概述

Workflow 基于 DAG（有向无环图）设计，支持：
- 多节点类型（Start、Task、Condition、Parallel、Validate、End 等）
- 条件分支与并行执行
- 模板变量替换（`{{var}}`）
- 验证规则引擎
- 执行状态持久化
- 失败重试策略

---

## 文件位置

Workflow YAML 文件可放置在两个目录：

| 目录 | 作用域 | 说明 |
|-----|-------|------|
| `.matrix/workflows/` | 项目级 | 仅当前项目可用 |
| `~/.matrix/workflows/` | 用户级 | 所有项目可用（全局） |

**优先级**：项目目录 > 用户目录（同名 workflow 项目级优先）

---

## YAML 格式规范

### 基本结构

```yaml
id: workflow-id            # 必填：唯一标识符
name: Workflow Name        # 必填：显示名称
version: "1.0.0"           # 可选：版本号
description: 描述文本       # 可选：功能说明

inputs:                    # 可选：输入参数定义
  - name: param_name
    type: string
    required: true
    default: "default_value"

outputs:                   # 可选：输出定义
  - name: result
    value: "{{output_var}}"

variables:                 # 可选：全局变量
  key: "value"

nodes:                     # 必填：节点列表
  - id: node_id
    type: start
    name: Node Name
    # ... 其他配置

edges:                     # 必填：边列表（连接节点）
  - from: node_a
    to: node_b
    condition: "{{var}} == 'value'"  # 可选条件

default_failure_strategy: abort  # 可选：默认失败策略
timeout_ms: 60000          # 可选：全局超时（毫秒）
```

---

## 节点类型

### 1. Start（开始节点）

每个 workflow 必须有且仅有一个 start 节点。

```yaml
- id: start
  type: start
  name: 开始
```

### 2. End（结束节点）

每个 workflow 必须有且仅有一个 end 节点。

```yaml
- id: end
  type: end
  name: 结束
```

### 3. Task（任务节点）

执行具体任务，支持多种任务类型。

```yaml
- id: process
  type: task
  name: 处理数据
  task: ai                  # 任务类型：ai / tool / custom
  params:                   # 任务参数（支持模板）
    prompt: "处理 {{input}}"
    model: "claude-sonnet"
  on_failure:               # 失败策略
    type: retry             # abort / retry / skip / continue
    max_attempts: 3
    interval_ms: 1000
  timeout_ms: 30000         # 节点超时
```

**任务类型说明**：

| 类型 | 说明 | 参数示例 |
|-----|------|---------|
| `ai` | 调用 AI 模型 | `prompt`, `model`, `temperature` |
| `tool` | 执行工具 | `tool_name`, `tool_params` |
| `custom` | 自定义任务 | 自定义参数 |

### 4. Condition（条件节点）

根据条件选择分支执行。

```yaml
- id: check_type
  type: condition
  name: 检查类型
  branches:
    - name: 文本分支
      condition: "{{type}} == 'text'"
      target: text_process
    - name: 图片分支
      condition: "{{type}} == 'image'"
      target: image_process
    - name: 默认分支
      condition: "default"    # 无匹配时执行
      target: fallback
```

### 5. Parallel（并行节点）

同时执行多个分支。

```yaml
- id: parallel_process
  type: parallel
  name: 并行处理
  parallel_branches:
    - id: branch_a
      name: 分支A
      nodes:
        - id: task_a
          type: task
          task: ai
          params:
            prompt: "处理A"
    - id: branch_b
      name: 分支B
      nodes:
        - id: task_b
          type: task
          task: ai
          params:
            prompt: "处理B"
  wait_ms: 5000             # 可选：等待超时
```

### 6. Validate（验证节点）

使用规则引擎验证状态。

```yaml
- id: validate_output
  type: validate
  name: 验证输出
  rules:
    - type: equals
      field: status
      value: "success"
    - type: contains
      field: result
      value: "expected_text"
  on_failure:
    type: abort
```

### 7. Wait（等待节点）

等待指定时间或外部条件。

```yaml
- id: wait_approval
  type: wait
  name: 等待审批
  wait_ms: 60000            # 等待60秒
  approvers:                # 可选：审批人
    - user_a
    - user_b
```

### 8. Subworkflow（子流程节点）

调用另一个 workflow。

```yaml
- id: call_sub
  type: subworkflow
  name: 调用子流程
  workflow: "another-workflow-id"
  params:
    input_var: "{{parent_var}}"
```

---

## 边与条件

边定义节点间的连接关系和执行顺序。

### 基本边

```yaml
edges:
  - id: e1
    from: start
    to: process
```

### 条件边

仅当条件满足时执行：

```yaml
edges:
  - id: e2
    from: check
    to: text_process
    condition: "{{type}} == 'text'"
    label: "文本路径"
```

**条件语法**：

| 操作符 | 示例 | 说明 |
|-------|------|------|
| `==` | `"{{var}} == 'value'"` | 等于 |
| `!=` | `"{{var}} != 'value'"` | 不等于 |
| `>` | `"{{count}} > 5"` | 大于 |
| `<` | `"{{count}} < 10"` | 小于 |
| `>=` | `"{{count}} >= 5"` | 大于等于 |
| `<=` | `"{{count}} <= 10"` | 小于等于 |
| `contains` | `"contains {{text}} 'keyword'"` | 包含 |
| `matches` | `"matches {{text}} '^pattern'"` | 正则匹配 |

---

## 变量与模板

### 全局变量

```yaml
variables:
  project_name: "MatrixCode"
  default_model: "claude-sonnet"
  max_tokens: 4096
```

### 模板替换

使用 `{{var}}` 语法定义模板，执行时自动替换：

```yaml
params:
  prompt: "你好 {{user_name}}，欢迎使用 {{project_name}}"
  model: "{{default_model}}"
```

**可用变量源**：

| 来源 | 访问方式 | 说明 |
|-----|---------|------|
| 输入参数 | `{{input_name}}` | workflow 调用时传入 |
| 全局变量 | `{{var_name}}` | variables 定义 |
| 前序输出 | `{{node_id.output}}` | 前序节点的输出结果 |
| 系统变量 | `{{sys.path}}` | 系统环境变量 |

---

## 输入输出

### 输入定义

```yaml
inputs:
  - name: user_input        # 参数名
    type: string            # 类型：string / number / boolean / object / array
    required: true          # 是否必填
    default: "默认值"        # 默认值（required=false 时有效）
    description: 用户输入内容  # 参数说明
```

### 输出定义

```yaml
outputs:
  - name: final_result      # 输出名
    value: "{{process.output}}"  # 输出值（支持模板）
    description: 最终处理结果    # 输出说明
```

---

## 执行策略

### 失败策略（on_failure）

| 策略 | 说明 | 适用场景 |
|-----|------|---------|
| `abort` | 立即终止整个 workflow | 关键任务失败 |
| `retry` | 重试当前节点 | 临时性错误（网络、API超时） |
| `skip` | 跳过当前节点继续执行 | 非关键任务 |
| `continue` | 继续执行下一节点 | 可接受的失败 |

**重试配置**：

```yaml
on_failure:
  type: retry
  max_attempts: 3          # 最大重试次数
  interval_ms: 1000        # 重试间隔（毫秒）
  backoff: exponential     # 重试策略：fixed / exponential
```

### 超时配置

```yaml
# 全局超时
timeout_ms: 300000        # 5分钟

# 节点超时
nodes:
  - id: long_task
    timeout_ms: 60000     # 1分钟
```

---

## 持久化

Workflow 执行状态自动持久化：

| 目录 | 说明 |
|-----|------|
| `.matrix/workflows/.instances/` | 项目级 workflow 实例 |
| `~/.matrix/workflows/.instances/` | 用户级 workflow 实例 |

**实例文件结构**：

```json
{
  "instance_id": "uuid",
  "workflow_id": "workflow-id",
  "status": "running|completed|failed|paused",
  "current_node": "node_id",
  "execution_path": ["start", "node1", "node2"],
  "variables": { "key": "value" },
  "error": null,
  "started_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}
```

---

## 调用方式

### 用户指令

在 CLI 或 TUI 中使用 `/workflow` 指令：

```bash
# 发现可用 workflow
/workflow discover

# 根据意图匹配
/workflow match "处理文本"

# 运行指定 workflow
/workflow run hello-world

# 带参数运行
/workflow run hello-world --inputs '{"user_name": "张三"}'

# 查看运行状态
/workflow status <instance-id>
```

### AI 工具调用

AI 可通过以下工具调用 workflow：

#### workflow_discover

发现可用 workflow：

```json
{
  "name": "workflow_discover",
  "parameters": {}
}
```

返回：
```
发现 3 个 workflow:

• hello-world - Hello World Workflow [需要: user_name]
• text-processor - 文本处理流程
• code-review - 代码审查流程
```

#### workflow_run

执行 workflow：

```json
{
  "name": "workflow_run",
  "parameters": {
    "workflow_id": "hello-world",
    "inputs": {
      "user_name": "张三"
    }
  }
}
```

返回：
```
Workflow 'hello-world' 执行结果:

实例ID: uuid-xxx
节点执行: 3 个
✓ 完成

变量输出: {
  "greeting": "你好，张三！欢迎使用 MatrixCode。"
}
```

#### workflow_match

根据意图匹配 workflow：

```json
{
  "name": "workflow_match",
  "parameters": {
    "query": "我想处理一些文本内容"
  }
}
```

返回：
```
匹配 '我想处理一些文本内容' 的 workflow:

• text-processor - 文本处理流程
• content-analyzer - 内容分析流程

调用: workflow_run {"workflow_id": "选定的ID"}
```

---

## 示例

### 简单问候流程

```yaml
id: hello-world
name: Hello World Workflow
description: 简单问候流程示例
version: "1.0.0"

inputs:
  - name: user_name
    type: string
    required: false
    default: "用户"

variables:
  greeting_template: "你好，欢迎使用 MatrixCode！"

nodes:
  - id: start
    type: start
    name: 开始

  - id: greet
    type: task
    name: 生成问候
    task: ai
    params:
      prompt: "请向 {{user_name}} 说一句友好的问候语，不超过20字。风格：{{greeting_template}}"

  - id: end
    type: end
    name: 结束

edges:
  - from: start
    to: greet
  - from: greet
    to: end

outputs:
  - name: greeting
    value: "{{greet.output}}"
```

### 条件分支流程

```yaml
id: content-processor
name: 内容处理流程
description: 根据内容类型选择处理方式

inputs:
  - name: content_type
    type: string
    required: true
  - name: content
    type: string
    required: true

nodes:
  - id: start
    type: start
    name: 开始

  - id: check_type
    type: condition
    name: 检查类型
    branches:
      - name: 文本处理
        condition: "{{content_type}} == 'text'"
        target: text_process
      - name: 代码处理
        condition: "{{content_type}} == 'code'"
        target: code_process
      - name: 默认
        condition: "default"
        target: fallback

  - id: text_process
    type: task
    name: 处理文本
    task: ai
    params:
      prompt: "分析以下文本内容：\n{{content}}"

  - id: code_process
    type: task
    name: 处理代码
    task: ai
    params:
      prompt: "审查以下代码：\n{{content}}"

  - id: fallback
    type: task
    name: 默认处理
    task: ai
    params:
      prompt: "处理以下内容：\n{{content}}"

  - id: end
    type: end
    name: 结束

edges:
  - from: start
    to: check_type
  - from: text_process
    to: end
  - from: code_process
    to: end
  - from: fallback
    to: end
```

### 并行执行流程

```yaml
id: parallel-analysis
name: 并行分析流程
description: 同时执行多种分析

inputs:
  - name: target_file
    type: string
    required: true

nodes:
  - id: start
    type: start
    name: 开始

  - id: parallel_check
    type: parallel
    name: 并行检查
    parallel_branches:
      - id: security_check
        name: 安全检查
        nodes:
          - id: security_scan
            type: task
            task: ai
            params:
              prompt: "检查 {{target_file}} 的安全漏洞"

      - id: quality_check
        name: 质量检查
        nodes:
          - id: quality_scan
            type: task
            task: ai
            params:
              prompt: "检查 {{target_file}} 的代码质量"

      - id: doc_check
        name: 文档检查
        nodes:
          - id: doc_scan
            type: task
            task: ai
            params:
              prompt: "检查 {{target_file}} 的文档完整性"

  - id: summarize
    type: task
    name: 汇总结果
    task: ai
    params:
      prompt: "汇总以下分析结果：\n安全: {{security_scan.output}}\n质量: {{quality_scan.output}}\n文档: {{doc_scan.output}}"

  - id: end
    type: end
    name: 结束

edges:
  - from: start
    to: parallel_check
  - from: parallel_check
    to: summarize
  - from: summarize
    to: end

outputs:
  - name: analysis_report
    value: "{{summarize.output}}"
```

---

## 最佳实践

### 1. 命名规范

- `id`: 使用小写字母和连线（`text-processor`）
- `name`: 简洁描述性名称
- `node.id`: 描述性名称（`check_type`、`generate_output`）

### 2. 模块化设计

- 复用逻辑封装为 subworkflow
- 每个节点职责单一
- 避免节点过多（建议 < 20 个）

### 3. 错误处理

- 关键节点配置 `on_failure: abort`
- 网络/API 调用配置 `retry` 策略
- 非关键节点使用 `skip` 或 `continue`

### 4. 测试验证

- 使用 Validate 节点检查关键输出
- 在条件节点添加 default 分支
- 合理设置超时时间

### 5. 文档说明

- 添加 `description` 描述 workflow 功能
- 为 inputs 添加说明
- 标注 required 参数

---

## 相关资源

- [Workflow 模块源码](../packages/core/src/workflow/)
- [测试示例](../packages/core/src/workflow/mod.rs#integration_tests)
- [规则引擎文档](../packages/core/src/workflow/rule_engine.rs)

---

**版本**: v1.0.0  
**更新**: 2026-05-25