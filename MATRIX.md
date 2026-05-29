```markdown
# MATRIX.md

> MatrixCode 项目概览 - AI 代码助手核心参考

## 项目定位

**可定制工作流的 AI 代码助手** - 通过 YAML 定义自动化流程，支持多模型协作、跨会话记忆、完全开源。

核心价值：
- **工作流定制** - YAML 定义任务流程，条件分支/并行执行
- **跨会话记忆** - 项目决策、技术选型、编码偏好持久化
- **成本优化** - 多模型分工，压缩用小模型节省 50-70% token
- **开源私有化** - MIT 协议，数据本地存储

---

## 技术栈

| 类别 | 技术 |
|------|------|
| **语言** | Rust 2024 Edition |
| **异步运行时** | Tokio (full features) |
| **HTTP 客户端** | reqwest (rustls) |
| **序列化** | serde, serde_json |
| **CLI 框架** | clap (derive) |
| **TUI 框架** | ratatui + crossterm |
| **数据库** | rusqlite (bundled) |
| **AI Provider** | Anthropic Claude / OpenAI GPT |

---

## 架构要点

### Workspace 结构

```
packages/
├── core/          # 核心引擎（Agent、MCP、Memory、Tools）
├── cli/           # 命令行接口（REPL、Terminal Mode）
└── tui/           # 终端用户界面（交互式 Dashboard）
```

### 核心模块

| 模块 | 路径 | 职责 |
|------|------|------|
| **Agent** | `core/src/agent/` | 任务规划、执行循环、工具调度 |
| **MCP** | `core/src/mcp/` | Model Context Protocol 集成 |
| **Memory** | `core/src/memory/` | 跨会话记忆存储与检索 |
| **Session** | `core/src/session/` | 会话管理、上下文压缩 |
| **Tools** | `core/src/tools/` | 内置工具（Bash、Edit、Glob、LSP 等） |
| **Prompt** | `core/src/prompt/` | 系统提示词构建与优化 |
| **Compress** | `core/src/compress/` | 上下文压缩算法 |
| **Providers** | `core/src/providers/` | AI 提供商抽象层 |
| **Workflow** | `tui/src/workflow/` | YAML 工作流引擎 |
| **LSP** | `core/src/lsp/` | Language Server Protocol 集成 |

### 多模型架构

```
┌─────────────────────────────────────────────┐
│                  MatrixCode                 │
├─────────────────────────────────────────────┤
│  MODEL_NAME (主模型)     → 任务执行          │
│  PLAN_MODEL (规划模型)   → 任务分解          │
│  COMPRESS_MODEL (压缩)   → 上下文摘要        │
│  FAST_MODEL (快速模型)   → 分类/提取         │
└─────────────────────────────────────────────┘
```

---

## 关键配置

### 环境变量 (`.env`)

```bash
# 提供商选择
PROVIDER=anthropic              # anthropic | openai

# API 配置
API_KEY=sk-ant-xxx
MODEL_NAME=claude-sonnet-4-20250514

# 多模型模式
MULTI_MODEL=true
COMPRESS_MODEL=claude-3-5-haiku-20241022
FAST_MODEL=claude-3-5-haiku-20241022
```

### 工作流定义 (`workflows/*.yaml`)

```yaml
# 示例: hello-world.yaml
steps:
  - name: "greet"
    action: "bash"
    command: "echo 'Hello from MatrixCode!'"
```

---

## 入口点

| 场景 | 入口 |
|------|------|
| CLI 主程序 | `packages/cli/src/main.rs` |
| TUI 应用 | `packages/tui/src/lib.rs` → `app.rs` |
| Core 库 | `packages/core/src/lib.rs` |
| VSCode 扩展 | `packages/vscode/src/extension.ts` |

---

## 数据目录

```
.openmatrix/
├── approvals/       # 工具调用审批记录
├── debug/           # 调试日志 (JSON)
├── logs/            # 运行日志
├── run-*/           # 工作流执行实例
│   ├── plan.md      # 执行计划
│   ├── tasks/       # 任务执行详情
│   └── state.json   # 状态快照
└── current.json     # 当前运行状态
```

---

## 扩展能力

- **自定义 Skills** - `skills/` 目录下定义技能模块
- **MCP Tools** - 通过 `mcp.example.toml` 配置外部工具
- **VSCode 集成** - WebView 聊天面板 + 会话管理
- **工作流模板** - `packages/core/workflows/` 预置模板

---

## 版本

- **当前版本**: 0.4.24
- **仓库**: https://github.com/bigfish1913/matrixcode
- **协议**: MIT
```