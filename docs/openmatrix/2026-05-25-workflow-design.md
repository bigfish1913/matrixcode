# 设计方案: 轻量级 Workflow 流程系统

日期: 2026-05-25

## 核心目标

- 程序硬性执行的任务流程，而非依赖 AI 自我约束
- 支持四种节点类型：AI 调用、工具执行、条件分支、验证
- 混合验证机制：简单规则程序验证，复杂判断 AI 验证
- YAML 文件定义 workflow，易于编写和维护
- 可配置失败策略：自动重试、人工介入、fallback

## 架构设计

### 整体架构

```
┌─────────────────┐
│  YAML Workflow  │  ← 定义文件
└────────┬────────┘
         │ 解析
         ▼
┌─────────────────┐
│  WorkflowEngine │  ← 状态机引擎
│  ├─ NodeGraph   │  ← 节点图（DAG）
│  ├─ Context     │  ← 运行时上下文（变量、历史输出）
│  └─ Executor    │  ← 节点执行器分发
└────────┬────────┘
         │
    ┌────┴────┬────────┬────────┐
    ▼         ▼        ▼        ▼
┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│AIExec│ │ToolEx│ │Cond  │ │Valid │
└──────┘ └──────┘ └──────┘ └──────┘
```

### 状态机流程

```
Running → 执行当前节点 → 处理结果 → 状态转换 → 循环/结束
   │
   ├─ Success → 存储输出 → 查找下一节点 → 继续
   │
   ├─ Failed → 失败策略处理
   │   ├─ Retry → 重试当前节点
   │   ├─ Human → 暂停等待人工介入
   │   ├─ Fallback → 跳转到 fallback 节点
   │   └─ Abort → 结束为 Failed
   │
   └─ Paused → 暂停等待恢复
```

## 数据模型

### WorkflowDef（静态定义）

```rust
pub struct WorkflowDef {
    pub name: String,
    pub description: String,
    pub nodes: HashMap<String, NodeDef>,
    pub edges: Vec<EdgeDef>,
    pub entry: String,
}
```

### NodeDef（节点定义）

```rust
pub struct NodeDef {
    pub id: String,
    pub node_type: NodeType,
    pub inputs: HashMap<String, InputSource>,
    pub outputs: HashMap<String, OutputTarget>,
    pub on_failure: FailureStrategy,
    pub retry_config: Option<RetryConfig>,
}

pub enum NodeType {
    Ai { prompt: String, model: Option<String> },
    Tool { tool_name: String, params: Value },
    Condition { expression: String, branches: HashMap<String, String> },
    Validate { rules: Vec<ValidationRule>, ai_validate: Option<String> },
}
```

### WorkflowContext（运行时状态）

```rust
pub struct WorkflowContext {
    pub variables: HashMap<String, Value>,
    pub node_outputs: HashMap<String, HashMap<String, Value>>,
    pub current_node: String,
    pub status: WorkflowStatus,
    pub history: Vec<NodeExecutionLog>,
}

pub enum WorkflowStatus {
    Running,
    Paused,
    Completed,
    Failed,
}
```

## YAML 定义格式

```yaml
name: code-review-workflow
description: 代码审查工作流

nodes:
  get_diff:
    type: tool
    tool: bash
    command: "git diff HEAD~1"
    outputs:
      git_diff: "stdout"

  analyze:
    type: ai
    prompt: |
      分析以下代码变更，识别潜在问题：
      {{changes}}
    inputs:
      changes: "$.git_diff"
    outputs:
      issues: "analysis_result.issues"

  check_issues:
    type: condition
    expression: "issues.length > 0"
    branches:
      true: generate_report
      false: success_end

  validate_report:
    type: validate
    rules:
      - "output.contains('建议')"
      - "output.length >= 100"
    on_failure: retry
    retry:
      max_attempts: 3
      prompt_adjust: "请更详细地分析"

edges:
  - from: get_diff
    to: analyze
  - from: analyze
    to: check_issues
```

## 关键接口

### NodeExecutor Trait

```rust
pub trait NodeExecutor {
    fn execute(
        &self,
        node: &NodeDef,
        inputs: HashMap<String, Value>,
        context: &mut WorkflowContext,
    ) -> Result<NodeResult>;
}
```

### CLI 命令

```bash
matrixcode workflow run <yaml_file>
matrixcode workflow list
matrixcode workflow status <id>
matrixcode workflow resume <id>
```

## 技术方案

### 新增模块

```
packages/core/src/workflow/
├── mod.rs             # WorkflowDef, NodeDef, 公共类型
├── engine.rs          # WorkflowEngine 状态机
├── executors.rs       # NodeExecutor trait + 各类型执行器
├── context.rs         # WorkflowContext
├── parser.rs          # YAML 解析器
└── rule_engine.rs     # 验证规则引擎
```

### 依赖

| 类别 | 选择 | 用途 |
|------|------|------|
| YAML 解析 | serde_yaml | 解析 workflow 定义文件 |
| 模板渲染 | handlebars 或自实现 | 渲染提示词模板 |
| 表达式解析 | 自实现 | 简单条件表达式和规则 |
| 序列化 | serde | WorkflowContext 持久化 |

### 复用现有系统

- **Provider**: AI 节点执行器调用现有 providers/
- **Tools**: 工具节点执行器调用现有 tools/
- **Skills**: 共存，不冲突

## 错误处理策略

### 失败策略

| 策略 | 行为 |
|------|------|
| Retry | 自动重试，最多 N 次，可选调整提示词 |
| Human | 暂停 workflow，等待人工介入 |
| Fallback | 跳转到指定的 fallback 节点继续执行 |
| Abort | 直接终止 workflow，状态为 Failed |

### 验证失败处理

1. 程序规则验证失败 → 触发节点失败策略
2. AI 验证失败 → 触发节点失败策略
3. 所有验证通过 → 进入下一节点

## 测试策略

### 单元测试

- YAML 解析器测试：各种格式的 workflow 定义
- 规则引擎测试：表达式解析和验证
- 各执行器测试：模拟输入输出

### 集成测试

- 完整 workflow 执行测试：从 YAML 到完成
- 失败策略测试：各种失败场景
- 暂停恢复测试：人工介入后恢复

## 约束与风险

### 约束

- 不引入外部状态机框架，保持轻量
- YAML 格式保持简单，避免过度复杂
- 与现有 Skills 系统共存，不替代

### 风险

- 表达式解析复杂度：控制表达式语法范围，只支持简单比较和逻辑运算
- AI 验证延迟：异步处理，不阻塞主流程
- 上下文大小：限制历史输出存储，可配置清理策略

## 验收标准

1. YAML workflow 文件可正确解析执行
2. 四种节点类型均可正常工作
3. 失败策略按配置正确处理
4. CLI `/workflow` 命令可用
5. 支持暂停和恢复
6. 单元测试和集成测试覆盖