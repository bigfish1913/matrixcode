# MatrixCode 项目概览

## 项目定位

**MatrixCode** 是一个可定制工作流的 AI 代码助手，核心特性：
- **YAML 工作流定义** - 通过 YAML 文件定义自动化任务流程，支持条件分支、并行执行
- **跨会话记忆** - 记住项目决策、技术选型、编码偏好
- **多模型分工** - 支持 Planning/Compression/Fast 等不同模型角色，优化成本
- **完全开源** - MIT 协议，数据本地存储，支持国内代理

## 技术栈

| 类别 | 技术 |
|------|------|
| 语言 | Rust 2024 Edition |
| 异步运行时 | Tokio |
| HTTP 客户端 | Reqwest (rustls) |
| CLI 框架 | Clap, Rustyline |
| TUI 框架 | Ratatui, Crossterm |
| 序列化 | Serde, Serde JSON |
| 数据库 | Rusqlite (bundled) |
| Token 计数 | tiktoken-rs |
| 错误处理 | Anyhow |

## 架构要点

### Workspace 结构
```
matrixcode/
├── packages/
│   ├── core/     # 核心库: Agent, Tools, MCP, LSP, Session, Memory
│   ├── cli/      # 命令行入口: 命令解析、终端交互
│   └── tui/      # 终端UI: 交互式界面、Markdown渲染
├── skills/       # 技能模块: code-review, git-commit, demo
└── src/          # 入口点与 Provider 实现
```

### 核心模块

| 模块 | 职责 |
|------|------|
| `agent` | Agent 循环、消息处理、工具调用、压缩策略 |
| `providers` | AI 提供商抽象 (Anthropic/OpenAI)、流式响应 |
| `tools` | 内置工具集 (Bash, Edit, Glob, LSP, MCP 等) |
| `session` | 会话持久化、恢复、历史管理 |
| `compress` | 上下文压缩、Token 优化 |
| `workflow` | YAML 工作流引擎 |
| `memory` | 跨会话记忆存储 |
| `skills` | 可扩展技能系统 |

### Provider 抽象
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat_stream(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>>;
    fn supports_caching(&self) -> bool;
}
```
- 支持 Anthropic 和 OpenAI
- 统一的消息格式 (`Message`, `ContentBlock`)
- 流式响应处理

### Agent 循环
1. 构建系统提示 (Prompt Profile + Skills + Project Overview)
2. 发送请求至 Provider (支持 Extended Thinking)
3. 处理响应: 文本块 → Markdown 渲染, 工具调用 → 执行
4. 上下文管理: 超过阈值触发压缩
5. 循环直到完成或达到最大迭代次数

### 多模型配置
```rust
struct MultiModelConfig {
    main: ModelConfig,       // 主模型
    plan: Option<ModelConfig>,  // 任务规划
    compress: Option<ModelConfig>, // 上下文压缩
    fast: Option<ModelConfig>,   // 快速操作
}
```

### 工具系统
- **内置工具**: Bash, Edit, Read, Write, Glob, LS, LSP
- **MCP 工具**: 通过 Model Context Protocol 接入外部工具
- **Server Tools**: 服务端工具 (如 Web Search)
- **Skills**: 可插拔技能模块

## 关键配置

```bash
# Provider 设置
PROVIDER=anthropic                    # anthropic | openai
API_KEY=sk-ant-xxx
MODEL_NAME=claude-sonnet-4-20250514

# 多模型模式
MULTI_MODEL=true
PLAN_MODEL=claude-sonnet-4-20250514
COMPRESS_MODEL=claude-3-5-haiku       # 压缩用小模型节省成本
FAST_MODEL=claude-3-5-haiku

# 上下文管理
COMPRESSION_THRESHOLD=0.8             # 触发压缩的上下文比例
```

## 入口点

- **CLI**: `packages/cli/src/main.rs` - 命令行参数解析
- **Core**: `packages/core/src/lib.rs` - 核心库导出
- **TUI**: `packages/tui/src/lib.rs` - 终端 UI 组件

## 扩展机制

### 添加新 Skill
```
skills/
└── my-skill/
    └── SKILL.md    # 技能定义文件
```

### 添加新 Provider
```rust
// src/providers/my_provider.rs
impl Provider for MyProvider {
    async fn chat_stream(&self, req: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>> {
        // 实现流式响应
    }
}
```

### 定义工作流
```yaml
# workflows/my-workflow.yaml
name: code-review
steps:
  - name: analyze
    tool: glob
    args: { pattern: "**/*.rs" }
  - name: review
    tool: ask
    prompt: "Review the files..."
```