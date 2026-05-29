# Workflow 制作指南

MatrixCode 现在支持 AI 帮助用户创建和修改 workflow，无需手动编写 YAML。

## 核心特性

### 1. 完整的创建-编辑循环

`workflow_create` 工具支持以下模式：

- **create**: 创建新 workflow
- **edit**: 编辑现有 workflow（精确修改）
- **template**: 获取预定义模板
- **validate**: 验证结构合法性
- **info**: 查看详细信息

### 2. 精确编辑操作

支持以下编辑操作：

| 操作 | 说明 | 示例 |
|------|------|------|
| `add_node` | 添加节点 | 添加新任务节点 |
| `remove_node` | 删除节点 | 删除中间节点 |
| `update_node` | 更新节点属性 | 修改名称、任务、参数 |
| `add_edge` | 添加连接 | 连接两个节点 |
| `remove_edge` | 删除连接 | 断开节点连接 |
| `add_input` | 添加输入参数 | 添加必需参数 |
| `update_metadata` | 更新元数据 | 修改名称、描述 |

### 3. AI 引导式创建

系统提示词内置了详细的 workflow 制作指导，AI 会：

- 主动识别用户需求（多步骤、重复性任务）
- 引导式创建（不直接问，而是建议）
- 分步展示，让用户参与调整
- 使用 info 模式展示结构化信息（而非复杂 YAML）

## 使用场景示例

### 场景 1：用户描述多步骤任务

```
用户: "我经常需要：先搜索相关资料，然后分析内容，最后生成报告"

AI: 
  → 识别：这是典型的多步骤自动化任务
  → 建议："这个流程很适合自动化！我可以帮你创建一个 workflow..."
  
  → 获取模板：
  workflow_create {"mode": "template", "template_type": "research"}
  
  → 创建初始版本：
  workflow_create {"mode": "create", "workflow": {
    "id": "research-report",
    "name": "Research and Report Generator",
    "nodes": [
      {"id": "start", "type": "start", "name": "Start"},
      {"id": "search", "type": "task", "name": "Search Materials", "task": "websearch"},
      {"id": "analyze", "type": "task", "name": "Analyze Content", "task": "analyze"},
      {"id": "generate", "type": "task", "name": "Generate Report", "task": "content_generation"},
      {"id": "end", "type": "end", "name": "End"}
    ],
    "edges": [...]
  }}
  
  → 展示结构：
  workflow_create {"mode": "info", "workflow_id": "research-report"}
  
  → 输出：
  📋 Workflow: research-report
  
  📊 Nodes:
    - start [start] Start
    - search [task] Search Materials
      Task: websearch
    - analyze [task] Analyze Content
      Task: analyze
    - generate [task] Generate Report
      Task: content_generation
    - end [end] End
  
  🔗 Edges:
    start → search
    search → analyze
    analyze → generate
    generate → end
  
  → 询问："workflow 创建好了！需要调整吗？比如添加并行搜索？"
```

### 场景 2：迭代修改

```
用户: "把搜索步骤改成并行搜索三个来源"

AI: 
  → 识别：需要改为并行节点
  
  → 第一步：修改节点类型
  workflow_create {
    "mode": "edit",
    "workflow_id": "research-report",
    "edit_operation": "update_node",
    "edit_target": "search",
    "edit_value": {
      "type": "parallel",
      "name": "Parallel Search"
    }
  }
  
  → 第二步：添加并行分支
  workflow_create {
    "mode": "edit",
    "workflow_id": "research-report",
    "edit_operation": "add_node",
    "edit_value": {
      "id": "search_google",
      "type": "task",
      "name": "Search Google",
      "task": "websearch",
      "params": {"query": "{{inputs.query}}"}
    }
  }
  
  → （继续添加其他搜索节点）
  
  → 展示更新后的结构：
  workflow_create {"mode": "info", "workflow_id": "research-report"}
  
  → 输出：
  ✓ Workflow 'research-report' updated successfully!
  
  📊 Nodes:
    - start [start] Start
    - search [parallel] Parallel Search
      - search_google [task] Search Google
      - search_bing [task] Search Bing
      - search_duckduckgo [task] Search DuckDuckGo
    - analyze [task] Analyze Content
    ...
```

### 场景 3：添加参数和验证

```
用户: "添加一个必需的查询参数"

AI: 
  workflow_create {
    "mode": "edit",
    "workflow_id": "research-report",
    "edit_operation": "add_input",
    "edit_value": {
      "name": "query",
      "type": "string",
      "required": true,
      "description": "Search query for research"
    }
  }
  
  → 验证结构：
  workflow_create {"mode": "validate", "workflow_id": "research-report"}
  
  → 输出：
  ✓ Workflow validation passed
  
  Nodes: 6
  Edges: 5
  Inputs: 1 (query [string] required)
  Outputs: 1 (report)
```

## 对比：传统方式 vs AI 辅助

### 传统方式（用户手动编写）

```yaml
id: research-report
name: Research and Report Generator
version: "1.0.0"

inputs:
  - name: query
    type: string
    required: true
    description: Search query

nodes:
  - id: start
    type: start
    name: Start
  
  - id: search
    type: parallel
    name: Parallel Search
    parallel_branches:
      - name: Google Search
        nodes:
          - id: search_google
            type: task
            name: Search Google
            task: websearch
            params:
              query: "{{inputs.query}}"
      # ... 用户需要手动编写大量 YAML
  
edges:
  - from: start
    to: search
  # ... 容易出错，难以维护
```

**问题**：
- YAML 语法复杂，容易出错
- 不了解 workflow 结构，无从下手
- 修改困难，需要手动定位
- 无法验证结构正确性

### AI 辅助方式

```
用户: "帮我创建一个搜索-分析-生成报告的 workflow，可以并行搜索多个来源"

AI: 自动创建 → 展示结构 → 询问调整

用户: "添加错误处理节点"

AI: 自动添加 → 验证结构 → 展示更新

用户: "修改节点名称为英文"

AI: 精准修改 → 即时生效
```

**优势**：
- 用户无需学习 YAML 语法
- AI 引导式创建，逐步完善
- 精确编辑，避免格式错误
- 自动验证，确保结构正确
- 结构化展示，清晰直观

## 最佳实践

### AI 行为准则

1. **主动识别需求**
   - 用户说"我经常需要..." → 建议自动化
   - 用户说"每次都要..." → 建议创建 workflow
   - 用户说"先做A，再做B，最后C" → 建议多步骤 workflow

2. **引导式对话**
   - ✅ "这个任务很适合自动化！我可以帮你创建..."
   - ❌ "你知道 workflow 吗？"
   - ✅ 用 info 模式展示结构
   - ❌ 直接展示复杂 YAML

3. **迭代式调整**
   - 创建后立即展示结构
   - 询问是否需要调整
   - 准备好多次 edit 操作
   - 每次 edit 后验证

4. **清晰命名**
   - ✅ "Data Validation" （意图清晰）
   - ❌ "task1" （无意义）

5. **主��建议**
   - 发现优化点时主动提出
   - "我建议添加错误处理节点..."
   - "这里可以用并行节点加速..."

### 技术实现要点

1. **直接文件操作**
   - edit/info 模式直接从文件加载
   - 避免 WorkflowRegistry 扫描延迟
   - 即时生效，无需重新发现

2. **结构验证**
   - 每次 edit 后自动验证
   - 确保节点连接完整性
   - 检查 start/end 节点存在

3. **精确修改**
   - 使用 edit_target 定位
   - 只修改指定部分
   - 保持其他部分不变

## 总结

MatrixCode 的 workflow 制作功能让用户无需学习复杂语法，通过自然对话即可创建和修改自动化流程。AI 会主动识别需求、引导创建、精确编辑，让 workflow 制作变得简单高效。