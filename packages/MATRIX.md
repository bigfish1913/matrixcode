# MatrixCode 项目概览

## 项目简介

MatrixCode 是一个用 Rust 编写的 AI 代码助手，支持多模型（Claude/GPT）、工具调用、会话管理、记忆系统和上下文压缩。

---

## 架构概览

```
┌─────────────────────────────────────────────────────────┐
│                      用户界面层                          │
│   ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│   │   CLI    │  │   TUI    │  │   VSCode Extension   │  │
│   └────┬─────┘  └────┬─────┘  └──────────┬───────────┘  │
└────────┼─────────────┼───────────────────┼───────────────┘
         │             │                   │
         └─────────────┴─────────┬─────────┘
                                   ▼
┌─────────────────────────────────────────────────────────┐
│                     Core 核心层                          │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌───────────────┐  │
│  │  Agent  │ │ Session │ │ Memory  │ │   Workflow    │  │
│  └────┬────┘ └────┬────┘ └────┬────┘ └───────┬───────┘  │
│       │          │          │               │           │
│  ┌────┴──────────┴──────────┴───────────────┴────┐      │
│  │              Tools (bash/edit/grep/...)       │      │
│  └────────────────────────┬──────────────────────┘      │
│                           ▼                              │
│  ┌────────────────────────────────────────────────────┐  │
│  │     Providers (Anthropic Claude / OpenAI GPT)      │  │
│  └────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 关键目录说明

| 目录 | 作用 |
|------|------|
| `cli/` | 命令行入口，包含命令处理、守护进程、服务模式等 |
| `core/` | 核心业务逻辑，包含 Agent、Memory、Session、Tools、Workflow、Compress 等 |
| `tui/` | 终端用户界面，处理绘制、事件、工作流可视化 |
| `vscode/` | VSCode 扩展，提供 WebView 聊天面板、配置管理、会话管理 |
| `npm/` | NPM 发布脚本，处理跨平台二进制下载 |
| `docs/` | 项目文档，包含代码审查、安全测试、改进清单等 |
| `website/` | 项目官网静态页面 |
| `.codegraph/` | 代码图数据库，用于代码分析工具 |

---

## 核心业务逻辑

### 1. Agent 运行流程
```
用户输入 → Agent.run() → 调用 Provider → 解析响应 → 
执行工具调用 → 流式输出 → 循环直到完成
```
- **builder.rs**: Agent 构建器，配置模型、工具、记忆等
- **run.rs**: 核心执行循环
- **streaming.rs**: 流式响应处理

### 2. 会话系统
- **session.rs**: 会话数据结构，存储消息历史
- **manager.rs**: 会话生命周期管理（创建、持久化、恢复）
- **metadata.rs**: 会话元数据（创建时间、模型、状态）

### 3. 记忆系统
- **manager.rs**: 记忆管理器，协调长期和短期记忆
- **project.rs**: 项目级记忆（概览、关键信息）
- **learning.rs**: 从对话中学习和提取知识
- **keywords.json**: 关键词提取配置

### 4. 工具调用
支持的工具类型：
- **文件操作**: `edit`, `ls`, `glob`, `grep`
- **执行**: `bash` 命令执行
- **交互**: `ask` 用户询问
- **代码分析**: `codegraph` 代码图查询
- **网络**: `websearch` 网络搜索
- **自动化**: `workflow` 工作流执行

### 5. 上下文压缩
- **compressor.rs**: 主压缩逻辑
- **phase_detector.rs**: 检测对话阶段
- **summarizer.rs**: 生成摘要
- **scorer.rs**: 消息重要性评分

### 6. 工作流引擎
- **parser.rs**: YAML 工作流解析
- **engine.rs**: 工作流执行引擎
- **executors/**: 各类执行器实现
- **rule_engine.rs**: 条件规则引擎

---

## 常用开发命令

```bash
# 构建
cargo build --release                    # 构建所有包
cargo build -p matrixcode-cli --release  # 仅构建 CLI

# 运行
cargo run -p matrixcode-cli -- --help    # 运行 CLI

# 测试
cargo test                               # 运行所有测试
cargo test -p matrixcode-core            # 仅测试核心模块
cargo test --test test_bash              # 运行特定测试

# 发布
./scripts/release.sh                     # CLI 发布脚本

# VSCode 扩展
cd vscode && npm install && npm run compile

# 格式化
cargo fmt
cargo clippy
```

---

## 关键模式和约定

### 1. 模块组织
- 每个 `mod.rs` 作为模块入口，导出公共 API
- 子模块按职责划分（如 `tools/`, `memory/`）

### 2. 配置管理
- 全局配置: `~/.matrix/config.json`
- 工作流配置: `workflows/*.yaml`
- 支持 JSON 和 YAML 格式

### 3. 错误处理
- 使用 `Result<T, E>` 进行错误传播
- 自定义错误类型在各模块的 `types.rs` 中定义

### 4. 异步模式
- 使用 Tokio 运行时
- 流式处理使用 `futures::Stream`

---

## 开发注意事项

1. **API 密钥安全**: 不要硬编码 API 密钥，使用配置文件或环境变量

2. **会话持久化**: 会话存储在 `~/.matrix/sessions/`，注意并发访问

3. **工具执行安全**: bash 工具有审批机制 (`approval.rs`)，敏感操作需确认

4. **上下文长度**: 长对话会自动压缩，注意关键信息保留

5. **Provider 兼容**: 添加新 Provider 需实现统一接口，参考 `providers/anthropic.rs`

6. **VSCode 扩展**: 需要与 CLI 通过 IPC 通信，参考 `matrixcodeClient.ts`

---

## 扩展指南

### 添加新工具
1. 在 `core/src/tools/` 创建模块
2. 实现工具 trait，定义 `name`, `description`, `execute`
3. 在 `mod.rs` 注册工具

### 添加新 Provider
1. 在 `core/src/providers/` 添加实现
2. 实现统一的请求/响应接口
3. 处理流式和非流式响应

### 创建工作流
1. 在 `workflows/` 创建 YAML 文件
2. 定义步骤、条件、变量
3. 参考 `conditional-example.yaml`