# MatrixCode 项目概览

## 项目简介

**MatrixCode** 是一个用 Rust 编写的可定制工作流 AI 代码助手，支持 YAML 定义自动化流程、多模型协作、跨会话记忆。采用 Cargo workspace 架构，包含 CLI、TUI、VSCode 扩展、NPM 包等多端实现。

---

## 核心架构

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend Layer                          │
│   ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌─────────────┐   │
│   │   CLI   │  │   TUI   │  │  VSCode  │  │     NPM     │   │
│   └────┬────┘  └────┬────┘  └────┬─────┘  └──────┬──────┘   │
└────────┼────────────┼────────────┼────────────────┼──────────┘
         │            │            │                │
         └────────────┴────────────┴────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│                    packages/core                             │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌──────────┐          │
│  │  agent  │ │ workflow │ │  tools  │ │  memory  │          │
│  └─────────┘ └──────────┘ └─────────┘ └──────────┘          │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ ┌──────────┐          │
│  │providers │ │ compress │ │ session │ │approval  │          │
│  └──────────┘ └──────────┘ └─────────┘ └──────────┘          │
└─────────────────────────────────────────────────────────────┘
```

---

## 关键目录说明

| 目录 | 作用 |
|------|------|
| `packages/core/` | 核心库：Agent 逻辑、工作流引擎、工具集、记忆系统、Provider 抽象 |
| `packages/cli/` | 命令行前端：命令处理器、显示模块、终端交互模式 |
| `packages/tui/` | 终端 UI：基于 ratatui 的交互界面、工作流可视化 |
| `packages/vscode/` | VSCode 扩展：WebView 聊天面板、会话管理、配置同步 |
| `packages/npm/` | NPM 封装：Node.js 集成层，下载脚本 |
| `.openmatrix/` | 运行时数据：审批记录、调试日志、工作流执行状态、任务记录 |
| `skills/` | 技能模块：代码审查、Git 提交、示例模板 |
| `workflows/` | 工作流定义：YAML 格式的自动化任务配置 |

---

## 常用开发命令

```bash
# 构建
cargo build                  # 构建所有包
cargo build --release        # 发布构建

# 运行
cargo run -p matrixcode-cli  # 运行 CLI
cargo run -p matrixcode-tui # 运行 TUI

# 测试
cargo test                   # 运行所有测试
cargo test -p matrixcode-core --test test_bash  # 单独测试

# 代码检查
cargo clippy                 # Lint 检查
cargo fmt                    # 格式化代码

# VSCode 扩展开发
cd packages/vscode && npm install && npm run compile
```

---

## 关键模式与约定

### 1. 多模型配置模式
项目支持四类模型分工协作：
- **主模型 (MODEL_NAME)**: 核心任务执行
- **规划模型 (PLAN_MODEL)**: 任务分解与步骤规划
- **压缩模型 (COMPRESS_MODEL)**: 上下文摘要（推荐小模型节省成本）
- **快速模型 (FAST_MODEL)**: 分类、提取等轻量操作

### 2. 工作流系统
- YAML 定义工作流，位于 `packages/core/workflows/`
- 运行时状态存储在 `.openmatrix/run-{timestamp}-{id}/`
- 支持：条件分支、并行执行、失败回滚

### 3. 会话记忆系统
- 基于 SQLite 持久化 (`rusqlite`)
- 存储于 `.openmatrix/` 目录
- 支持跨会话决策记录、技术偏好记忆

### 4. 技能系统
- 每个技能目录包含 `SKILL.md` 定义
- 位于 `skills/` 目录，支持自定义扩展

---

## 核心业务流程

### 工作流执行流程
```
用户触发 → 加载 YAML 工作流 → 规划模型分解任务 
    → 任务队列 → 主模型执行 → 工具调用 
    → 审批检查 → 结果持久化 → 压缩模型摘要
```

### Agent 工具调用
`packages/core/tools/` 提供核心工具：
- `bash` - Shell 命令执行
- `edit` - 代码编辑
- `ls/glob` - 文件检索
- `codegraph` - 代码图谱分析

### 审批机制
`.openmatrix/approvals/` 存储用户审批记录，危险操作需显式确认。

---

## 开发注意事项

1. **环境配置**: 复制 `.env.example` 为 `.env`，配置 `API_KEY` 和 `PROVIDER`

2. **多提供商支持**: 支持 Anthropic 和 OpenAI，可通过 `BASE_URL` 配置代理

3. **运行时目录**: `.openmatrix/` 包含敏感数据，需添加到 `.gitignore`

4. **VSCode 调试**: 参考 `docs/VSCODE_DEBUG.md` 配置 launch.json

5. **文档迁移**: `docs/migration/` 记录架构演进，新功能参考现有设计文档

6. **测试策略**: `packages/core/tests/` 包含集成测试，使用 mockito 模拟外部 API

7. **版本同步**: workspace version 在根 `Cargo.toml` 统一管理，当前版本 `0.4.18`