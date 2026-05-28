# MatrixCode 项目概览

## 项目简介

**MatrixCode** 是一个基于 Rust 开发的可定制工作流 AI 代码助手。核心特性包括：
- **YAML 工作流引擎** - 通过声明式配置定义自动化任务流程
- **跨会话记忆** - 持久化项目决策、技术选型、编码偏好
- **多模型分工** - 支持 Anthropic/OpenAI，压缩用小模型节省 50-70% token
- **MCP 协议支持** - 可扩展工具集成

版本: `0.4.23` | 许可证: MIT | 平台: Windows/macOS/Linux

---

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                    CLI Entry Point                       │
│              (packages/cli/src/main.rs)                  │
└────────────────────┬────────────────────────────────────┘
                     │
         ┌───────────┴───────────┐
         ▼                       ▼
┌─────────────────┐    ┌─────────────────┐
│   TUI Module    │    │   Core Engine   │
│  (packages/tui) │    │ (packages/core) │
└─────────────────┘    └────────┬────────┘
                                │
        ┌───────────┬───────────┼───────────┬───────────┐
        ▼           ▼           ▼           ▼           ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ Agent   │ │ Session │ │  MCP    │ │ Memory  │ │Workflow │
   │ Engine  │ │ Manager │ │ Client  │ │ Store   │ │ Engine  │
   └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘
        │           │           │           │           │
        └───────────┴───────────┴───────────┴───────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
              ┌──────────┐           ┌──────────┐
              │Providers  │           │  Tools   │
              │Anthropic/ │           │ Bash/Edit│
              │OpenAI     │           │ Glob/LS  │
              └──────────┘           └──────────┘
```

**核心模块职责**：
| 模块 | 路径 | 职责 |
|------|------|------|
| Agent | `core/src/agent/` | AI 代理执行引擎，管理对话循环 |
| Session | `core/src/session/` | 会话状态管理，消息历史 |
| Memory | `core/src/memory/` | 跨会话记忆存储（SQLite） |
| Workflow | `core/src/workflow/` | YAML 工作流解析与执行 |
| MCP | `core/src/mcp/` | Model Context Protocol 客户端 |
| Compress | `core/src/compress/` | 上下文压缩，token 优化 |
| Providers | `core/src/providers/` | AI 提供商适配器 |
| Tools | `core/src/tools/` | 内置工具（Bash/Edit/Glob/LS 等） |

---

## 关键目录说明

### 源码目录
| 目录 | 作用 |
|------|------|
| `packages/core/` | 核心库，包含所有业务逻辑 |
| `packages/cli/` | CLI 入口，命令解析与用户交互 |
| `packages/tui/` | 终端 UI（ratatui 实现） |
| `packages/vscode/` | VSCode 扩展 |
| `packages/npm/` | NPM 发布包 |

### 配置与数据目录
| 目录 | 作用 |
|------|------|
| `.openmatrix/` | 运行时数据目录 |
| `.openmatrix/run-*/` | 单次执行记录 |
| `.openmatrix/memory/` | 跨会话记忆存储 |
| `.openmatrix/approvals/` | 危险操作审批记录 |
| `skills/` | 技能定义（code-review/git-commit 等） |
| `workflows/` | 内置工作流 YAML 文件 |

### 文档目录
| 目录 | 作用 |
|------|------|
| `docs/migration/` | 架构迁移文档 |
| `docs/openmatrix/` | 设计文档（压缩优化/TUI/工作流） |

---

## 常用开发命令

### 构建命令
```bash
# 构建所有包
cargo build --workspace

# 构建发布版本
cargo build --workspace --release

# 仅构建 CLI
cargo build -p matrixcode-cli --release

# 仅构建核心库
cargo build -p matrixcode-core
```

### 测试命令
```bash
# 运行所有测试
cargo test --workspace

# 运行核心库测试
cargo test -p matrixcode-core

# 运行特定测试
cargo test -p matrixcode-core test_bash

# 运行集成测试
cargo test -p matrixcode-core --test test_mcp_core
```

### 运行命令
```bash
# 开发模式运行
cargo run -p matrixcode-cli

# 直接运行（需先构建）
./target/release/matrixcode

# 交互式终端模式
cargo run -p matrixcode-cli -- terminal

# 执行工作流
cargo run -p matrixcode-cli -- workflow run hello-world.yaml
```

### 发布命令
```bash
# NPM 包发布
cd packages/npm && npm publish

# VSCode 扩展打包
cd packages/vscode && vsce package
```

---

## 关键模式与约定

### 工作流模式（Workflow Pattern）
```yaml
# 示例：workflows/hello-world.yaml
name: hello-world
steps:
  - name: greet
    action: bash
    command: echo "Hello, MatrixCode!"
  - name: conditional
    action: condition
    when: "{{steps.greet.success}}"
    then:
      - action: bash
        command: echo "Condition met!"
```

**工作流特性**：
- 条件分支
- 并行执行（parallel）
- 失败重试
- 变量插值（`{{variable}}`）

### 多模型分工模式
```bash
# .env 配置
MULTI_MODEL=true
MODEL_NAME=claude-sonnet-4-20250514    # 主模型（执行）
PLAN_MODEL=claude-sonnet-4-20250514      # 规划模型
COMPRESS_MODEL=claude-3-5-haiku-20241022 # 压缩模型（节省成本）
FAST_MODEL=claude-3-5-haiku-20241022     # 快速模型（分类/提取）
```

### 记忆模式
- **Session Memory**: 当前会话的短期记忆
- **Project Memory**: 项目级别的长期记忆（技术栈、约定）
- **User Memory**: 用户偏好（编码风格、常用命令）

### 工具注册模式
```rust
// 内置工具注册位置：core/src/tools/
// 扩展工具通过 MCP 协议注册
// tools/registry.rs 统一管理工具发现与调用
```

---

## 业务逻辑详解

### 1. 任务执行流程
```
用户输入 → CLI 解析 → Agent 接收
    ↓
Session 创建/加载 → 加载 Memory
    ↓
消息发送给 Provider (Anthropic/OpenAI)
    ↓
响应解析 → 工具调用判断
    ↓
┌─ 普通响应 → 输出给用户
└─ 工具调用 → 执行工具 → 返回结果 → 循环
    ↓
会话压缩（超过阈值时）→ 保存 Memory → 结束
```

### 2. 上下文压缩机制
**触发条件**：token 超过 `MAX_CONTEXT_TOKENS` 阈值

**压缩策略**：
1. 保留系统提示和最近消息
2. 中间对话摘要化
3. 使用小模型生成摘要

**配置**：
```bash
COMPRESSION_THRESHOLD=0.75    # 75% 时触发
MAX_CONTEXT_TOKENS=200000    # 最大上下文
```

### 3. 工作流执行流程
```
加载 YAML → 解析步骤 → 构建执行图
    ↓
按顺序/并行执行步骤
    ↓
每个步骤：
  - 变量插值
  - 条件判断
  - 工具调用
  - 结果收集
    ↓
处理失败（重试/跳过/终止）
    ↓
输出最终结果
```

### 4. MCP 协议集成
**服务配置**（`mcp.example.toml`）：
```toml
[mcp.playwright]
command = "npx"
args = ["-y", "@executeautomation/playwright-mcp-server"]

[mcp.filesystem]
command = "uvx"
args = ["mcp-server-filesystem", "/path/to/allowed"]
```

**执行流程**：
1. 启动时加载 MCP 配置
2. 通过 JSON-RPC 与 MCP Server 通信
3. 动态发现 MCP 提供的工具
4. 将 MCP 工具注入 Agent 工具列表

### 5. 审批机制
**危险操作**需要用户确认：
- 文件删除
- 系统命令执行
- 网络请求

**流程**：
```
工具调用 → 检查风险级别
    ↓
┌─ 低风险 → 直接执行
└─ 高风险 → 请求审批 → 用户确认/拒绝 → 记录日志
```

---

## 开发注意事项

### 环境配置
```bash
# 1. 复制配置文件
cp .env.example .env

# 2. 设置 API Key
# 方式一：直接设置
ANTHROPIC_API_KEY=sk-ant-xxx

# 方式二：通用变量
PROVIDER=anthropic
API_KEY=sk-ant-xxx

# 3. 国内代理（可选）
BASE_URL=https://your-proxy.com
```

### 代码约定
1. **错误处理**：使用 `anyhow::Result`，避免 `unwrap()`
2. **异步运行时**：统一使用 `tokio`
3. **日志规范**：`log` crate + `env_logger`
4. **测试位置**：`core/tests/` 下按功能命名 `test_*.rs`

### 常见问题
1. **编译慢**：启用增量编译，使用 `sccache`
2. **测试失败**：检查 `.env` 配置，确保 API Key 有效
3. **MCP 连接失败**：确认 MCP Server 已安装（`npx`/`uvx`）

### 扩展开发
**添加新工具**：
1. 在 `core/src/tools/` 创建模块
2. 实现 `Tool` trait
3. 在 `tools/registry.rs` 注册

**添加新 Provider**：
1. 在 `core/src/providers/` 创建模块
2. 实现 `Provider` trait
3. 在 `providers/mod.rs` 注册

---

## 核心文件索引

| 文件 | 作用 |
|------|------|
| `packages/cli/src/main.rs` | CLI 入口 |
| `packages/core/src/agent/` | AI 代理核心逻辑 |
| `packages/core/src/workflow/` | 工作流引擎 |
| `packages/core/src/memory/` | 记忆存储 |
| `packages/core/src/mcp/` | MCP 协议实现 |
| `.env.example` | 环境配置模板 |
| `workflows/*.yaml` | 内置工作流示例 |

---

## 快速开始

```bash
# 1. 克隆项目
git clone https://github.com/bigfish1913/matrixcode.git
cd matrixcode

# 2. 配置环境
cp .env.example .env
# 编辑 .env 设置 API_KEY

# 3. 构建
cargo build --release

# 4. 运行
./target/release/matrixcode "解释这个项目的架构"
```

---

**文档版本**: 基于 v0.4.23 | 最后更新: 2025-05