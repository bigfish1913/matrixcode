# MatrixCode

**可定制工作流的 AI 代码助手** - 通过 YAML 定义自动化流程，支持多模型、跨会话记忆、完全开源。

![Version](https://img.shields.io/badge/version-0.4.17-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows%20|%20macOS%20|%20Linux-lightgrey)

## 为什么选择 MatrixCode？

如果你需要：
- **可定制的工作流** - 用 YAML 定义自动化任务流程，不依赖 AI 自我约束
- **跨会话记忆** - 记住你的项目决策、技术选型、编码偏好
- **成本优化** - 多模型分工，压缩用小模型节省 50-70% token
- **开源私有化** - MIT 开源，数据本地存储，支持国内代理

那就选择 MatrixCode。

## MatrixCode vs Claude Code

| 功能 | Claude Code | MatrixCode |
|------|-------------|------------|
| **工作流定制** | ❌ 无 | ✅ **YAML 定义工作流**，条件分支、并行执行、失败重试 |
| **跨会话记忆** | ⚠️ 有但有限 | ✅ **完整记忆系统**：分类、评分、冲突检测、时间衰减 |
| **成本优化** | ❌ 单一模型 | ✅ 多模型协作，fast 模型处理压缩和简单任务，节省 50-70% |
| **开源** | ❌ 闭源 | ✅ **MIT 开源**，可自由修改和部署 |
| **多 Provider** | ❌ 仅 Anthropic | ✅ Anthropic + OpenAI + 国内代理 |
| **私有化部署** | ❌ 云端依赖 | ✅ **本地存储**，数据完全自主 |
| **跨平台** | ✅ | ✅ Linux/macOS/Windows |

### 简单对比示例

**Claude Code**：每次都需要重新告诉 AI 你的项目背景

```bash
# 第一次对话
> 这个项目用 TypeScript，后端是 Express...
[AI 记住当前会话]

# 关闭终端后，新会话
> 帮我写一个路由
[AI: 你需要先告诉我用什么框架...]  # 记忆丢失！
```

**MatrixCode**：自动记住你的决策

```bash
# 第一次对话
> 这个项目用 TypeScript，后端是 Express
[saved 2 memories: 技术决策]

# 关闭终端后，新会话
> 帮我写一个路由
[loaded 15 accumulated memories]
# AI 自动知道用 TypeScript + Express，不需要重复说明！
```

## 核心特性

### 📋 可定制工作流引擎

**这是 MatrixCode 最大的差异化功能。**

通过 YAML 文件定义自动化任务流程，程序硬性执行而非依赖 AI 自我约束：

```yaml
id: code-review-workflow
name: 代码审查流程
description: 自动分析代码变更并生成审查报告

inputs:
  - name: target_file
    type: string
    required: true

nodes:
  - id: start
    type: start
    
  - id: get_diff
    type: task
    task: tool
    params:
      tool_name: bash
      command: "git diff HEAD~1 -- {{target_file}}"
      
  - id: analyze
    type: task
    task: ai
    params:
      prompt: "分析以下代码变更，识别潜在问题：\n{{get_diff.output}}"
      
  - id: validate
    type: validate
    rules:
      - type: contains
        field: output
        value: "建议"
    on_failure:
      type: retry
      max_attempts: 3
      
  - id: end
    type: end

edges:
  - from: start
    to: get_diff
  - from: get_diff
    to: analyze
  - from: analyze
    to: validate
  - from: validate
    to: end
```

**节点类型**：
- `start/end` - 流程入口和出口
- `task` - 执行任务（AI 调用、工具执行）
- `condition` - 条件分支
- `parallel` - 并行执行
- `validate` - 规则验证
- `subworkflow` - 调用子流程

**失败策略**：
- `retry` - 自动重试，可配置次数和间隔
- `skip` - 跳过继续执行
- `abort` - 立即终止
- `fallback` - 跳转到备用节点

使用：
```bash
# 发现可用 workflow
/workflow discover

# 根据意图匹配
/workflow match "审查代码"

# 运行指定 workflow
/workflow run code-review-workflow --inputs '{"target_file": "src/main.rs"}'
```

### 🧠 跨会话记忆系统

SQLite 持久化存储，记忆跨会话保留：

**记忆类型**：
- 🎯 **决策**：技术选型决定（重要性 90）
- 👤 **偏好**：用户习惯（重要性 70）
- 💡 **发现**：重要信息（重要性 60）
- 🔧 **解决方案**：解决方法（重要性 85）

**智能特性**：
- **冲突检测**：`"改用 X"` 自动覆盖 `"使用 Y"`
- **时间衰减**：旧记忆重要性降低
- **关键词触发**：根据对话内容自动检索相关记忆
- **引用增加**：常用记忆重要性提升

### 💰 多模型协作 = 成本节省

配置多个模型分工协作：

```env
# 主任务用大模型
MODEL=claude-sonnet-4-20250514

# 快速模型处理压缩、简单判断等任务（成本 ≈ Sonnet 的 1/10）
FAST_MODEL=claude-3-5-haiku-20241022
```

**fast 模型用途**：
- 上下文压缩（高频操作）
- 简单分类和判断
- 记忆提取和检索
- 快速响应任务

对于长时间编程任务，可节省 **50-70% token 成本**。

### 🔧 丰富的工具系统

- **文件操作**：`read`、`write`、`edit`、`multi_edit`、`glob`、`grep`、`ls`
- **代码执行**：`bash`（安全沙箱）
- **代码知识**：`codegraph`（tree-sitter 代码图谱）
- **网络能力**：`webfetch`、`websearch`（DuckDuckGo、Wikipedia）
- **任务管理**：`todo_write`、`task`、`plan_mode`
- **工作流**：`workflow`（运行和发现工作流）

## 项目结构

```
matrixcode/
├── packages/
│   ├── core/              # 核心逻辑库 (Rust)
│   │   ├── agent/             # Agent 核心
│   │   ├── compress/          # 上下文压缩
│   │   ├── memory/            # 记忆系统
│   │   ├── workflow/          # 工作流引擎 ⭐
│   │   ├── tools/             # 工具系统
│   │   └── providers/         # API 提供者
│   │
│   ├── tui/               # Terminal UI (Rust)
│   ├── cli/               # CLI 入口 (Rust)
│   ├── vscode/            # VS Code 扩展
│   └── npm/               # npm 发布包
│
├── skills/                # 技能文件
├── docs/                  # 文档
├── Taskfile.yml           # Task 任务定义
└── Cargo.toml             # Rust Workspace
```

## 快速开始

### 安装

```bash
# npm（推荐）
npm install -g @bigfishnpm/matrixcode

# Cargo
cargo install matrixcode
```

### 配置

创建 `~/.matrix/config.json`（注意字段名使用 snake_case）：

```json
{
  "provider": "anthropic",
  "api_key": "your-api-key",
  "model": "claude-sonnet-4-20250514",
  "fast_model": "claude-3-5-haiku-20241022"
}
```

### 使用

```bash
# 终端交互模式
matrixcode

# 单次问答
matrixcode "分析项目结构"

# Daemon 服务模式（VS Code 扩展用）
matrixcode --mode daemon

# 会话管理
matrixcode --list-sessions
matrixcode --resume
```

## CLI 命令参数

### 全局参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-m, --mode` | 运行模式 | `terminal` |
| `-r, --resume` | 交互式恢复会话 | - |
| `--resume-id <ID>` | 恢复指定会话 | - |
| `--list-sessions` | 列出历史会话 | - |
| `-c, --continue-session` | 继续上次会话 | - |
| `--skills-dir <PATH>` | 额外技能目录 | - |
| `--think [true/false]` | 扩展思考模式 | 配置值 |
| `--max-tokens` | 最大输出 tokens | 16384 |

### 运行模式

| 模式 | 说明 |
|------|------|
| `terminal` / `tui` | 终端交互模式（TUI 界面） |
| `service` / `json` | JSON 输出模式（脚本集成） |
| `daemon` | Daemon 服务模式（VS Code 扩展） |

### 子命令

```bash
# 聊天
matrixcode chat --message "你的问题"

# 快速操作
matrixcode quick-action --action explain --file src/main.rs

# 新建会话
matrixcode new-session

# 查看历史
matrixcode history

# 查看状态
matrixcode status

# Workflow 命令
matrixcode workflow discover          # 发现可用 workflow
matrixcode workflow discover "审查"   # 搜索匹配的 workflow
matrixcode workflow run file.yaml     # 运行 workflow
matrixcode workflow run file.yaml --inputs '{"key": "value"}'
matrixcode workflow list              # 列出历史
matrixcode workflow list --status failed
matrixcode workflow status <id>       # 查看状态
matrixcode workflow resume <id>       # 恢复暂停的 workflow
matrixcode workflow abort <id>        # 终止运行中的 workflow
matrixcode workflow export file.yaml --format mermaid  # 导出流程图
```

### VS Code 扩展

1. 在 VS Code 扩展市场搜索 "MatrixCode"
2. 安装扩展
3. 按 `Ctrl+K` 打开聊天面板

快捷键：
- `Ctrl+K` - 打开聊天
- `Ctrl+Shift+E` - 解释代码
- `Ctrl+Shift+F` - 修复代码
- `Ctrl+Shift+T` - 生成测试
- `Ctrl+Shift+R` - 重构代码

## 开发

### 环境要求

- Rust 1.75+（2024 Edition）
- Node.js 18+（VS Code 扩展）

### 构建

```bash
# 查看所有任务
task --list

# 构建 CLI
task build

# 构建 VS Code 扩展
task build-vscode

# 运行测试
task test
```

### 发布

```bash
# 自动升级版本并发布
task publish

# 手动发布指定版本
task release -- 0.4.18
```

## 文档

- [工作流使用指南](docs/workflow-guide.md) - 详细 YAML 格式和节点说明
- [MatrixCode vs Claude Code](docs/MatrixCode_vs_Claude_Code.md) - 详细对比
- [CLI 使用指南](packages/cli/README.md)
- [Session 记忆架构](docs/SESSION_MEMORY_ARCHITECTURE.md)

## 贡献

欢迎贡献代码！查看 [CONTRIBUTING.md](CONTRIBUTING.md)。

提交规范（Conventional Commits）：
```
feat(scope): 新功能
fix(scope): Bug 修复
docs: 文档更新
refactor: 重构
```

## 许可证

MIT License

## 链接

- [GitHub](https://github.com/bigfish1913/matrixcode)
- [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=bigfish1913.matrixcode)
- [npm Package](https://www.npmjs.com/package/@bigfishnpm/matrixcode)
- [crates.io](https://crates.io/crates/matrixcode)