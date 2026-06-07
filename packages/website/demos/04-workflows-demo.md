# MatrixCode Workflows 工作流 Demo

Workflows 是 MatrixCode 的核心特性，提供自动化多步骤任务执行。

## 🎯 Demo 目标

展示如何：
1. 使用内置 Workflow
2. 创建自定义 Workflow
3. 运行并监控 Workflow

## 📋 Workflows 系统概述

### Workflow 结构

每个 Workflow 包含：
- **节点 (Nodes)**: 任务步骤
- **边 (Edges)**: 连接关系
- **输入参数 (Inputs)**: 需要的参数
- **输出 (Outputs)**: 生成结果

### Workflow 类型

| 类型 | 说明 | 节点示例 |
|------|------|---------|
| **start** | 开始节点 | 流程起点 |
| **task** | 任务节点 | 执行具体任务 |
| **condition** | 条件节点 | 根据条件分支 |
| **parallel** | 并行节点 | 并行执行多个任务 |
| **end** | 结束节点 | 流程终点 |

## 🚀 使用内置 Workflow

### 1. 查看可用 Workflows

**用户输入：**
```
列出所有可用的 workflows
```

**MatrixCode 响应：**
```
🔧 tool: workflow_discover
返回:

Available workflows:
1. test-simple
   Description: Simple Test Workflow
   Required inputs: none
   
2. book-publish
   Description: 自动化创建新的 mdBook 书籍、构建和发布的完整流程
   Required inputs: book_name, book_title
   
3. hello-world
   Description: 一个简单的测试工作流，演示基本功能
   Required inputs: none
   
4. image-article
   Description: Generate an illustrated article from one sentence
   Required inputs: topic
   
5. simple-article
   Description: Generate a simple article template and save to file
   Required inputs: topic
```

### 2. 运行 Workflow

**用户输入：**
```
使用 image-article workflow 生成一篇关于 "Rust 异步编程" 的文章
```

**MatrixCode 响应：**
```
💡 匹配 workflow: image-article

🔧 tool: workflow_run
参数: {
  workflow_id: "image-article",
  inputs: {
    topic: "Rust 异步编程"
  }
}

执行 workflow...

📋 Workflow: image-article
步骤:
1. 研究主题 → websearch
2. 搜索图片 → image_search
3. 生成文章 → content_generation
4. 保存文件 → write

执行中...

⏳ Step 1: 研究主题
🔍 websearch: "Rust async programming best practices"
✅ 找到 5 个相关资料

⏳ Step 2: 搜索图片
🔍 image_search: "rust async programming diagram"
✅ 找到 3 张图片

⏳ Step 3: 生成文章
✍️ content_generation:
  - 标题: Rust 异步编程完全指南
  - 章节: 5 个
  - 图片: 3 张
✅ 文章内容已生成

⏳ Step 4: 保存文件
✏️ write: rust_async_article.md
✅ 文件已保存

✅ Workflow 完成！
输出: rust_async_article.md (已生成带图片的文章)
```

### 3. 查看 Workflow 信息

**用户输入：**
```
查看 book-publish workflow 的详细信息
```

**MatrixCode 响应：**
```
🔧 tool: workflow_create
参数: {
  mode: "info",
  workflow_id: "book-publish"
}

返回:

📋 Workflow: book-publish
版本: 1.0.0
描述: 自动化创建新的 mdBook 书籍、构建和发布的完整流程

📊 节点:
  - start [start] Start
  - init_book [task] Initialize Book Structure
    Task: bash
    Params: command = "mdbook init {{book_name}}"
  - build [task] Build Book
    Task: bash
    Params: command = "mdbook build"
  - publish [task] Publish to GitHub Pages
    Task: bash
    Params: command = "gh-pages deploy"
  - end [end] End

🔗 连接:
  start → init_book
  init_book → build
  build → publish
  publish → end

📝 必需参数:
  - book_name: 书籍目录名
  - book_title: 书籍标题

💡 用法:
  workflow_run {
    workflow_id: "book-publish",
    inputs: {
      book_name: "my-rust-book",
      book_title: "My Rust Programming Guide"
    }
  }
```

## 🎯 创建自定义 Workflow

### 方式一：AI 帮助创建（推荐）

**用户输入：**
```
我经常需要：先分析代码，然后搜索最佳实践，最后生成优化建议
帮我创建一个自动化 workflow
```

**MatrixCode 响应：**
```
💡 这个流程很适合自动化！我来帮你创建一个 workflow...

🔧 tool: workflow_create
参数: {
  mode: "create",
  workflow: {
    id: "code-optimization",
    name: "Code Optimization Workflow",
    description: "分析代码并生成优化建议",
    inputs: [
      { name: "file_path", type: "string", description: "要分析的文件路径" }
    ],
    nodes: [
      { id: "start", type: "start", name: "Start" },
      { id: "analyze", type: "task", name: "Analyze Code", task: "read" },
      { id: "search", type: "task", name: "Search Best Practices", task: "websearch" },
      { id: "generate", type: "task", name: "Generate Suggestions", task: "content_generation" },
      { id: "save", type: "task", name: "Save Report", task: "write" },
      { id: "end", type: "end", name: "End" }
    ],
    edges: [
      { from: "start", to: "analyze" },
      { from: "analyze", to: "search" },
      { from: "search", to: "generate" },
      { from: "generate", to: "save" },
      { from: "save", to: "end" }
    ]
  }
}

✅ Workflow 已创建！

📋 Workflow: code-optimization

📊 节点:
  - start [start] Start
  - analyze [task] Analyze Code
    Task: read
    Params: path = "{{file_path}}"
  - search [task] Search Best Practices
    Task: websearch
    Params: query = "best practices for {{detected_language}}"
  - generate [task] Generate Suggestions
    Task: content_generation
  - save [task] Save Report
    Task: write
    Params: path = "optimization_report.md"
  - end [end] End

🔗 连接:
  start → analyze → search → generate → save → end

📝 输入参数:
  - file_path: 要分析的文件路径

💡 需要调整吗？比如添加并行搜索多个来源？
```

**用户反馈：**
```
添加一个并行搜索节点，同时搜索 3 个来源
```

**MatrixCode 响应：**
```
好的，我来修改...

🔧 tool: workflow_create
参数: {
  mode: "edit",
  workflow_id: "code-optimization",
  edit_operation: "update_node",
  edit_target: "search",
  edit_value: {
    type: "parallel",
    name: "Search Multiple Sources",
    branches: [
      { id: "search_google", task: "websearch", params: { query: "{{topic}} best practices" } },
      { id: "search_docs", task: "websearch", params: { query: "{{topic}} official docs" } },
      { id: "search_examples", task: "websearch", params: { query: "{{topic}} examples" } }
    ]
  }
}

✅ 已更新为并行搜索！

现在 search 节点会同时搜索：
  - Google 最佳实践
  - 官方文档
  - 示例代码

📊 更新后的结构:
  start → analyze → [并行搜索] → generate → save → end

需要运行测试吗？
```

### 方式二：手动编写 YAML

**文件：.matrix/workflows/my-workflow.yaml**

```yaml
id: my-workflow
name: My Custom Workflow
version: 1.0.0
description: 自定义工作流示例

inputs:
  - name: input_file
    type: string
    description: 输入文件路径

outputs:
  - name: output_file
    type: string
    description: 输出文件路径

nodes:
  - id: start
    type: start
    name: Start
    params: {}
    on_failure:
      type: abort

  - id: read_file
    type: task
    name: Read Input File
    task: read
    params:
      path: "{{input_file}}"
    on_failure:
      type: abort

  - id: process
    type: task
    name: Process Content
    task: analyze
    params:
      content: "{{read_file.output}}"
    on_failure:
      type: abort

  - id: write_output
    type: task
    name: Write Output
    task: write
    params:
      path: "{{output_file}}"
      content: "{{process.output}}"
    on_failure:
      type: abort

  - id: end
    type: end
    name: End
    params: {}
    on_failure:
      type: abort

edges:
  - id: edge_1
    from: start
    to: read_file

  - id: edge_2
    from: read_file
    to: process

  - id: edge_3
    from: process
    to: write_output

  - id: edge_4
    from: write_output
    to: end

variables: {}
default_failure_strategy:
  type: abort
```

## 🎯 Workflow 监控和调试

### 监控运行状态

**用户输入：**
```
查看正在运行的 workflow 状态
```

**MatrixCode 响应：**
```
🔧 tool: workflow_status
参数: { workflow_id: "current" }

返回:

📊 Workflow 运行状态:
Workflow: code-optimization
Status: running
Progress: 60%

当前步骤:
  ✅ Start (已完成)
  ✅ Analyze Code (已完成)
  ⏳ Search Best Practices (执行中...)
  ⏸️ Generate Suggestions (等待)
  ⏸️ Save Report (等待)
  ⏸️ End (等待)

执行时间: 45 秒
预计剩余: 30 秒
```

### 处理失败

**用户输入：**
```
workflow 执行失败了，查看错误信息
```

**MatrixCode 响应：**
```
🔧 tool: workflow_status
参数: { workflow_id: "code-optimization" }

返回:

❌ Workflow 执行失败:
Workflow: code-optimization
Status: failed
Failed at: Search Best Practices

错误信息:
  Error: websearch timeout
  Message: Search query exceeded 30s timeout

建议修复:
  1. 增加超时时间
  2. 简化搜索查询
  3. 使用缓存结果

是否重新运行？
```

**用户输入：**
```
修复并重新运行
```

**MatrixCode 响应：**
```
🔧 tool: workflow_create
参数: {
  mode: "edit",
  workflow_id: "code-optimization",
  edit_operation: "update_node",
  edit_target: "search",
  edit_value: {
    params: { timeout: 60 }
  }
}

✅ 已增加超时时间到 60 秒

🔧 tool: workflow_run
参数: {
  workflow_id: "code-optimization",
  inputs: { file_path: "src/main.rs" }
}

重新执行 workflow...
✅ 这次运行成功！
```

## ✨ Workflows 的优势

1. **自动化**: 一次配置，重复执行
2. **可视化**: 清晰的流程图结构
3. **并行处理**: 使用 parallel 节点提升效率
4. **条件分支**: 使用 condition 节点灵活决策
5. **错误处理**: 内置失败策略和恢复机制
6. **参数化**: 支持变量和模板
7. **AI 协助**: 自动创建和修改 workflow

## 🎯 实际应用场景

| 场景 | Workflow | 用途 |
|------|---------|------|
| 内容创作 | image-article | 研究主题、搜索图片、生成文章 |
| 书籍出版 | book-publish | 创建书籍、构建、发布 |
| 代码优化 | code-optimization | 分析、搜索、生成建议 |
| 批量处理 | batch-process | 处理多个文件 |
| 自动测试 | auto-test | 运行测试、生成报告 |

## 🔗 相关文档

- [Workflow 制作指南](../docs.html#workflow-creation)
- [节点类型详解](../docs.html#workflow-nodes)
- [内置 Workflows](../docs.html#builtin-workflows)

## 📊 测试验证

运行以下命令验证 Workflows 系统：

```bash
# 查看可用 workflows
matrixcode --list-workflows

# 运行测试 workflow
matrixcode
> 使用 hello-world workflow

# 创建自定义 workflow
matrixcode
> 帮我创建一个 workflow...
```

预期输出：
```
✅ Workflows discovered: 5
✅ Workflow execution started
✅ Workflow completed successfully
```