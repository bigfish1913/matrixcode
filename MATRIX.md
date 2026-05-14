# MATRIX.md - MatrixCode 项目概览

## 1. 项目简介

**MatrixCode** 是一个基于 Rust 开发的智能代码代理CLI 工具。它能够理解自然语言指令，并通过调用大型语言模型（LLM）如 OpenAI 或 Anthropic 来执行代码编写、文件操作、命令执行等任务。该项目采用了“工具调用”架构，允许 AI 智能体与本地文件系统、Shell 环境及网络进行交互，旨在成为一个可扩展、高效的开发辅助工具。

## 2. 核心架构

MatrixCode 采用了典型的 **Agent + Tool + Provider** 架构模式：

*   **Agent (核心代理)**: 负责管理对话上下文、构建提示词、调度 LLM 请求以及处理模型的响应。它维护了会话的历史状态，并负责解析模型的输出以决定是否调用工具。
*   **Provider (模型提供者)**: 抽象了不同 LLM API 的差异（如 OpenAI 和 Anthropic），处理 API 请求、流式响应解析以及认证。支持扩展新的模型提供商。
*   **Tool (工具集)**: 定义了代理可执行的操作。每个工具（如读写文件、执行 Bash、搜索代码）都实现了统一的 `Tool` trait，包含定义（JSON Schema）和执行逻辑。
*   **Workspace (工作区)**: 管理当前的工作目录上下文，为代理提供文件系统的访问边界。

## 3. 目录结构详解

```text
matrixcode/
├── src/
│   ├── main.rs           # 程序入口，CLI 参数解析，REPL 循环主逻辑
│   ├── lib.rs            # 库模块导出
│   ├── agent.rs          # Agent 核心实现，包含会话管理、工具调用循环
│   ├── providers/        # LLM 提供商实现
│   │   ├── mod.rs        # 定义 Provider trait 及通用数据结构 (Message, Role, ContentBlock)
│   │   ├── anthropic.rs  # Anthropic API 适配
│   │   └── openai.rs     # OpenAI API 适配
│   ├── tools/            # 工具实现模块
│   │   ├── mod.rs        # Tool trait 定义及工具注册
│   │   ├── read.rs       # 文件读取工具
│   │   ├── write.rs      # 文件写入工具
│   │   ├── edit.rs       # 文件编辑工具
│   │   ├── bash.rs       # Shell 命令执行工具
│   │   ├── glob.rs       # 文件匹配工具
│   │   ├── ls.rs         # 目录列出工具
│   │   ├── search.rs     # 代码/文本搜索工具
│   │   ├── todo_write.rs # 任务清单管理工具
│   │   ├── skill.rs      # 技能调用工具
│   │   └── webfetch/...  # 网络相关工具
│   ├── skills.rs         # 技能加载与管理逻辑
│   ├── prompt.rs         # 系统提示词构建与 Profile 管理
│   ├── markdown.rs       # 终端 Markdown 渲染逻辑
│   ├── overview.rs       # 项目概览生成逻辑
│   └── workspace.rs      # 工作区状态管理
├── tests/                # 集成测试目录
├── docs/                 # 文档目录
├── .env.example          # 环境变量配置模板
└── Cargo.toml            # Rust 项目配置文件
```

## 4. 关键功能模块

### 4.1 Agent (智能代理)

`src/agent.rs` 是项目的大脑。其主要职责包括：
*   **上下文管理**: 维护 `messages` 向量，保存对话历史，包括用户输入、模型回复、工具调用及结果。
*   **Prompt 构建**: 根据配置（`PromptProfile`）和项目概览（`ProjectOverview`）构建系统提示词。
*   **执行循环**: 实现了“思考-行动-观察”循环。发送请求 -> 接收流式响应 -> 解析工具调用 -> 执行工具 -> 将结果回传 -> 继续请求，直到模型返回 `end_turn` 或达到最大迭代次数。
*   **Token 统计**: 实时追踪输入/输出 Token 消耗。

### 4.2 Providers (模型接口)

位于 `src/providers/`，采用 Trait 抽象，支持多模型切换：
*   **Provider Trait**: 定义了 `chat_stream` 异步方法，返回流式事件。
*   **数据结构**:
    *   `Message`: 包含角色和内容。
    *   `ContentBlock`: 支持文本、工具调用、工具结果、思考块以及服务端工具。
*   **兼容性**: 目前已实现 Anthropic 和 OpenAI 接口适配。

### 4.3 Tools (工具系统)

位于 `src/tools/`，遵循统一的接口规范：
*   `definition()`: 返回工具的 JSON Schema 定义，告诉模型如何调用。
*   `execute()`: 执行具体的逻辑，返回字符串结果。
*   **内置工具**: 支持文件读写、多文件编辑、正则搜索、Glob 搜索、Bash 命令执行、任务清单管理等。

### 4.4 Skills (技能系统)

*   允许通过文件系统定义可复用的技能集（默认扫描 `./skills` 和 `~/.matrix/skills`）。
*   Agent 启动时会加载这些技能，并将其描述注入到系统提示词中，模型可通过 `skill` 工具调用特定的技能逻辑。

## 5. 业务逻辑流程

### 5.1 启动与初始化流程

1.  **解析参数**: 通过 `clap` 解析命令行参数（如 `--provider`, `--model`, `--resume`, `--init` 等）。
2.  **加载环境**: 从 `.env` 和环境变量中加载 API Key 等配置。
3.  **构建依赖**: 实例化 Provider（如 Anthropic 客户端）、Workspace 和 Agent。
4.  **技能加载**: 扫描指定目录加载技能列表。
5.  **概览生成/加载**:
    *   如果指定 `--init`，则调用 AI 分析项目结构并生成概览后退出。
    *   否则，尝试加载现有的项目概览注入上下文（除非指定 `--no-overview`）。
6.  **进入主循环**:
    *   若指定了 prompt 参数，执行单次问答后退出。
    *   否则，启动交互式 REPL（Read-Eval-Print Loop），等待用户输入。

### 5.2 交互式对话流程

1.  **用户输入**: 用户在终端输入指令。
2.  **请求构建**: Agent 将用户输入、系统提示词、历史对话记录打包成 `ChatRequest`。
3.  **流式响应**: Agent 调用 Provider 的流式接口。
4.  **实时渲染**:
    *   文本块：实时打印或渲染为 Markdown。
    *   思考块：以暗色斜体显示模型的内心独白。
5.  **工具处理**:
    *   当流中返回 `tool_use` 块时，Agent 暂停接收文本。
    *   根据工具名称查找本地工具实现。
    *   执行工具（如 `edit file`），获取结果。
    *   将结果封装为 `tool_result` 消息，加入对话历史。
    *   **递归调用**: 再次发起请求，让模型根据工具结果继续生成回复。
6.  **结束**: 当模型返回 `end_turn` 时，回合结束，等待下一次输入。

## 6. 开发指南

### 6.1 常用命令

```bash
# 构建项目
cargo build

# 运行项目 (需要配置 .env)
cargo run

# 运行测试
cargo test

# 生成项目概览
cargo run -- --init

# 指定模型运行
cargo run -- --provider anthropic --model claude-3-opus-20240229

# 运行并禁用 Markdown 渲染
cargo run -- --markdown false
```

### 6.2 配置说明

复制 `.env.example` 为 `.env` 并填写以下关键配置：

*   `PROVIDER`: 使用的模型提供商 (目前支持 "openai", "anthropic")。
*   `API_KEY`: 认证密钥。
*   `MODEL_NAME`: 模型名称 (如 "sonnet-4-20250514")。
*   `BASE_URL`: 可选，自定义 API 端点。
*   `THINK`: 是否开启扩展思考模式 (仅限支持的模型)。
*   `MAX_TOKENS`: 单次响应最大 Token 数。

### 6.3 扩展开发

**添加新工具**:
1.  在 `src/tools/` 下新建文件（如 `my_tool.rs`）。
2.  实现 `Tool` trait：
    ```rust
    #[async_trait]
    impl Tool for MyTool {
        fn definition(&self) -> ToolDefinition { ... }
        async fn execute(&self, params: Value) -> Result<String> { ... }
    }
    ```
3.  在 `src/tools/mod.rs` 的 `all_tools_with_skills` 函数中注册新工具。

**添加新 Provider**:
1.  在 `src/providers/` 下新建文件。
2.  实现 `Provider` trait，处理 HTTP 请求和响应流解析。
3.  在 `src/main.rs` 中根据参数实例化该 Provider。

## 7. 开发注意事项

1.  **异步运行时**: 项目基于 `tokio` 构建，所有涉及 I/O 的操作（网络请求、文件读写）均需使用 `async/await`。
2.  **错误处理**: 项目统一使用 `anyhow` 进行错误处理，便于在 CLI 中输出友好的错误信息。
3.  **敏感操作**: Bash 工具和文件写入工具具有潜在风险。在生产环境部署时应考虑添加权限控制或沙箱机制。
4.  **Token 消耗**: 频繁的工具调用和长上下文会迅速消耗 Token。建议合理设置 `MAX_TOKENS` 和对话历史截断策略（当前代码中主要通过 `TokenStats` 监控）。
5.  **版本兼容**: 项目 `Cargo.toml` 中指定 `edition = "2024"`，请确保使用的 Rust 编译器版本支持该版次。
6.  **测试隔离**: 集成测试中使用了 `mockito` 和 `tempfile`，确保测试不依赖外部网络环境和真实文件系统。