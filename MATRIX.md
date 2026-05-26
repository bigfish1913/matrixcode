# MatrixCode 项目概览

## 项目简介

**MatrixCode** 是一个基于 Rust 开发的智能代码代理 CLI 工具，核心定位是成为一个具备多模型支持、自动上下文压缩和任务规划能力的 AI 编程助手。

### 核心特性

| 特性 | 说明 |
|------|------|
| 🤖 多模型配置 | 支持 main/plan/compress/fast 四种模型角色分工 |
| 🧠 跨会话记忆 | 自动记住偏好、决策、发现，下次对话自动加载 |
| 🗜️ 智能压缩 | 自动检测并压缩超出阈值的上下文，避免 token 溢出 |
| 📋 任务规划 | 使用规划模型分解复杂任务，自动生成执行步骤 |
| 💾 会话管理 | 支持多会话保存、恢复、命名和交互式选择 |
| 📁 文件操作 | 读写、编辑、多文件编辑、搜索、模式匹配 |
| 🔧 工具系统 | 可扩展的工具架构，支持技能系统 |
| 🌐 Web 能力 | 内置网页搜索和网页抓取能力 |
| 💻 跨平台 | 支持 Linux、macOS、Windows |

---

## 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Layer (main.rs)                      │
│  参数解析 · 会话管理 · 交互循环 · 信号处理                          │
├─────────────────────────────────────────────────────────────────┤
│                        Agent Layer (agent.rs)                    │
│  对话循环 · 工具调度 · 审批控制 · 压缩触发 · 任务规划                │
├──────────────────────┬──────────────────────────────────────────┤
│    Providers Layer   │              Tools Layer                  │
│  ┌────────────────┐  │  ┌─────────────────────────────────────┐  │
│  │   Anthropic    │  │  │ Read │ Write │ Edit │ MultiEdit │  │  │
│  │   Provider     │  │  │ Bash │ Search │ Glob │ Ls │ Ask   │  │  │
│  ├────────────────┤  │  │ WebSearch │ WebFetch │ Skill │ Todo │  │
│  │    OpenAI      │  │  └─────────────────────────────────────┘  │
│  │   Provider     │  │                                          │
│  └────────────────┘  │                                          │
├──────────────────────┴──────────────────────────────────────────┤
│                    Supporting Systems                            │
│  Session · Memory · Compression · Approval · Cancel · Overview   │
├─────────────────────────────────────────────────────────────────┤
│                      Infrastructure                               │
│  Workspace · UI · Markdown · Prompt · Skills                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 目录结构详解

```
matrixcode/
├── .github/workflows/          # CI/CD 配置
│   ├── ci.yml                  # 持续集成：测试、构建
│   └── release.yml             # 发布流程：构建二进制、发布到 GitHub/npm
│
├── defaults/prompts/           # 默认提示词配置
│
├── docs/                       # 项目文档
│   ├── CI_CD.md               # CI/CD 说明文档
│   └── MatrixCode_vs_Claude_Code.md  # 产品对比文档
│
├── game/                       # 演示/测试用的小游戏
│   ├── game.js
│   ├── index.html
│   └── style.css
│
├── npm/                        # npm 发布包
│   ├── install.js             # 安装脚本（检测环境、下载二进制）
│   ├── package.json           # npm 包配置
│   └── README.md
│
├── src/                        # 源代码主目录
│   ├── providers/             # LLM 提供者实现
│   │   ├── mod.rs            # Provider trait 定义、消息类型
│   │   ├── anthropic.rs      # Anthropic Claude API 实现
│   │   └── openai.rs         # OpenAI API 实现
│   │
│   ├── tools/                 # 工具实现
│   │   ├── mod.rs            # Tool trait、工具注册
│   │   ├── read.rs           # 文件读取
│   │   ├── write.rs          # 文件写入
│   │   ├── edit.rs           # 单文件编辑
│   │   ├── multi_edit.rs     # 多文件批量编辑
│   │   ├── bash.rs           # Shell 命令执行
│   │   ├── search.rs         # 文件内容搜索
│   │   ├── glob.rs           # 文件名模式匹配
│   │   ├── ls.rs             # 目录列表
│   │   ├── ask.rs            # 用户提问
│   │   ├── websearch.rs      # Web 搜索
│   │   ├── webfetch.rs       # 网页抓取
│   │   ├── skill.rs          # 技能调用
│   │   ├── todo_write.rs     # 待办事项管理
│   │   └── spinner.rs        # UI 加载指示器
│   │
│   ├── agent.rs               # 核心代理实现（主循环）
│   ├── main.rs                # CLI 入口
│   ├── lib.rs                 # 库模块导出
│   ├── session.rs             # 会话持久化管理
│   ├── memory.rs              # 跨会话记忆系统
│   ├── compress.rs           # 上下文压缩策略
│   ├── models.rs             # 模型配置、任务规划
│   ├── approval.rs           # 操作审批机制
│   ├── cancel.rs             # 取消令牌（中断处理）
│   ├── workspace.rs          # 工作目录管理
│   ├── overview.rs           # 项目概览生成
│   ├── prompt.rs             # 系统提示词构建
│   ├── skills.rs             # 技能加载与管理
│   ├── ui.rs                 # 终端 UI 工具
│   └── markdown.rs           # Markdown 渲染
│
├── tests/                      # 集成测试
│   ├── test_bash.rs
│   ├── test_edit.rs
│   ├── test_providers.rs
│   └── ... (其他测试文件)
│
├── Cargo.toml                 # Rust 项目配置
├── .env.example               # 环境变量示例
└── README.md                  # 项目说明
```

---

## 核心模块分析

### 1. Provider 层 (`src/providers/`)

定义了与 LLM API 交互的统一抽象：

```rust
// 核心类型定义
pub struct Message {
    pub role: Role,           // System, User, Assistant, Tool
    pub content: MessageContent,
}

pub enum ContentBlock {
    Text { text: String },
    ToolUse { id, name, input },
    ToolResult { tool_use_id, content },
    Thinking { thinking, signature },  // Anthropic extended thinking
    ServerToolUse { id, name, input }, // 服务端工具（如 web_search）
    WebSearchResult { tool_use_id, content },
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest, tx: mpsc::Sender<StreamEvent>);
}
```

**支持的 Provider:**
- **Anthropic**: 支持 extended thinking、prompt caching、server-side web search
- **OpenAI**: 标准 chat completions 接口

### 2. Tools 层 (`src/tools/`)

所有工具实现统一的 `Tool` trait：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;  // 工具定义
    async fn execute(&self, params: Value) -> Result<String>;
    fn risk_level(&self) -> RiskLevel { RiskLevel::Safe }  // 风险等级
}
```

**工具风险等级分类：**

| 风险等级 | 工具 | 说明 |
|---------|------|------|
| Safe | read, ls, glob, search, ask | 只读操作，无需审批 |
| Low | websearch, webfetch, todo_write | 低风险操作 |
| Medium | edit, multi_edit, write, skill | 文件修改，可能需要审批 |
| High | bash | 命令执行，高风险，默认需审批 |

**内置工具列表：**

| 工具 | 功能描述 |
|------|---------|
| `ReadTool` | 读取文件内容，支持行号范围 |
| `WriteTool` | 创建或覆盖文件 |
| `EditTool` | 单文件字符串替换编辑 |
| `MultiEditTool` | 多文件批量编辑 |
| `BashTool` | 执行 Shell 命令 |
| `SearchTool` | 文件内容正则搜索 |
| `GlobTool` | 文件名模式匹配 |
| `LsTool` | 列出目录结构 |
| `AskTool` | 向用户提问 |
| `WebSearchTool` | Web 搜索 |
| `WebFetchTool` | 抓取网页内容 |
| `SkillTool` | 调用已加载的技能 |
| `TodoWriteTool` | 管理待办事项 |

### 3. Agent 核心 (`src/agent.rs`)

Agent 是系统的核心控制器，负责：
- 维护对话消息历史
- 调用 Provider API
- 调度工具执行
- 触发上下文压缩
- 处理用户审批

```rust
pub struct Agent {
    provider: Box<dyn Provider>,           // 主模型
    compress_provider: Option<Box<dyn Provider>>,  // 压缩模型
    plan_provider: Option<Box<dyn Provider>>,      // 规划模型
    tools: Vec<Box<dyn Tool>>,
    messages: Vec<Message>,                 // 对话历史
    memory_summary: Option<String>,         // 跨会话记忆
    project_overview: Option<String>,       // 项目概览
    compression_config: CompressionConfig,
    approve_mode: ApproveMode,
    // ...
}
```

**Agent 主循环流程：**

```
用户输入 → 构建 Message → 调用 Provider → 接收响应
                                              ↓
返回结果给用户 ← 压缩上下文(可选) ← 执行工具 ← 解析 tool_use
```

### 4. 会话与记忆系统

**会话管理 (`src/session.rs`):**
- 持久化对话历史到本地文件
- 支持会话命名、列表、恢复
- 自动保存/加载最近会话

**跨会话记忆 (`src/memory.rs`):**
- 自动累积用户偏好、决策、发现
- 每次启动自动加载记忆摘要
- 记忆存储在 `~/.matrix/memory/` 目录

### 5. 上下文压缩 (`src/compress.rs`)

当对话上下文超过阈值时，自动触发压缩：

```rust
pub struct CompressionConfig {
    pub threshold_tokens: u32,     // 触发压缩的阈值
    pub target_ratio: f32,          // 压缩目标比例
    pub preserve_recent_messages: usize,  // 保留最近 N 条消息
}
```

**压缩策略：**
- 保留系统提示和最近的对话
- 将历史对话总结为摘要
- 可使用专门的压缩模型（如 Haiku）降低成本

### 6. 任务规划 (`src/models.rs`)

支持复杂任务的自动分解：

```rust
pub struct TaskPlan {
    pub steps: Vec<TaskStep>,
    pub complexity: TaskComplexity,
}

pub enum TaskComplexity {
    Simple,      // 单步可完成
    Moderate,    // 2-5 步
    Complex,     // 5 步以上
}
```

**多模型角色配置：**

| 角色 | 用途 | 推荐模型 |
|------|------|---------|
| main | 主要对话和代码生成 | claude-sonnet-4 |
| plan | 任务分解和规划 | claude-sonnet-4 或 claude-opus-4 |
| compress | 上下文压缩 | claude-3-5-haiku |
| fast | 快速分类和提取 | claude-3-5-haiku |

---

## 配置系统

### 环境变量配置

```bash
# 基础配置
PROVIDER=anthropic                    # 提供者：anthropic 或 openai
API_KEY=sk-ant-xxx                   # API 密钥
MODEL_NAME=claude-sonnet-4-20250514  # 主模型名称
BASE_URL=                            # 自定义 API 端点（可选）

# 多模型模式
MULTI_MODEL=false                    # 启用多模型模式
PLAN_MODEL=                          # 规划模型（默认使用 MODEL_NAME）
COMPRESS_MODEL=                      # 压缩模型（推荐 haiku）
FAST_MODEL=                          # 快速模型

# 记忆系统
MEMORY_ENABLED=true                  # 启用跨会话记忆
MEMORY_MAX_ENTRIES=100              # 最大记忆条目数

# 压缩配置
COMPRESSION_THRESHOLD=80000          # 触发压缩的 token 阈值
COMPRESSION_TARGET_RATIO=0.3        # 压缩目标比例

# 审批模式
APPROVE_MODE=suggest                 # suggest（建议）/ auto（自动）/ manual（手动）

# 功能开关
THINK=true                          # 启用 Anthropic extended thinking
MARKDOWN=true                        # Markdown 渲染输出
```

### CLI 参数

```bash
matrixcode [OPTIONS]

选项:
  -p, --provider <NAME>       提供者
  -m, --model <NAME>          模型名称
  --api-key <KEY>            API 密钥
  --base-url <URL>           API 端点
  --think <BOOL>             启用 extended thinking
  --markdown <BOOL>          Markdown 渲染
  -c, --continue             继续上次会话
  --resume [ID]              恢复指定会话
  --list-sessions            列出所有会话
  --skills-dir <DIR>         额外技能目录
  --no-default-skills        禁用默认技能目录
  --profile <NAME>           提示词配置
```

---

## 常用开发命令

### 构建与运行

```bash
# 开发构建
cargo build

# 发布构建（优化）
cargo build --release

# 直接运行
cargo run -- [CLI 参数]

# 安装到系统
cargo install --path .
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_edit
cargo test test_providers

# 显示测试输出
cargo test -- --nocapture

# 运行单个测试文件
cargo test --test test_bash
```

### 发布流程

```bash
# 本地打包
cargo package

# 发布到 crates.io
cargo publish

# 构建 npm 包
cd npm && npm pack
```

### 调试与日志

```bash
# 启用日志输出
RUST_LOG=debug cargo run

# 指定模块日志
RUST_LOG=matrixcode::agent=trace cargo run
```

---

## 关键设计模式与约定

### 1. Builder 模式

Agent 使用 Builder 模式构建：

```rust
let agent = Agent::builder(provider)
    .think(true)
    .markdown(true)
    .profile(PromptProfile::Default)
    .skills(skills)
    .max_tokens(16384)
    .build();
```

### 2. Trait 抽象

- `Provider` trait：统一不同 LLM API
- `Tool` trait：统一工具接口
- 所有工具实现 `definition()` 和 `execute()` 方法

### 3. 异步架构

全异步实现，基于 Tokio：
- 使用 `async_trait` 定义异步 trait
- 流式响应通过 `mpsc` channel 传递
- 支持取消令牌中断长时间操作

### 4. 错误处理

统一使用 `anyhow::Result`，错误传播清晰。

### 5. 配置优先级

```
CLI 参数 > 环境变量 > .env 文件 > 默认值
```

---

## 开发注意事项

### 1. 添加新工具

1. 在 `src/tools/` 创建新文件
2. 实现 `Tool` trait
3. 在 `src/tools/mod.rs` 的 `all_tools_with_skills()` 中注册

```rust
// src/tools/my_tool.rs
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "my_tool".to_string(),
            description: "工具描述".to_string(),
            parameters: json!({ /* JSON Schema */ }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // 实现逻辑
        Ok("结果".to_string())
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium  // 根据风险设置
    }
}
```

### 2. 添加新 Provider

1. 在 `src/providers/` 创建新文件
2. 实现 `Provider` trait
3. 在 `src/providers/mod.rs` 导出并添加创建逻辑

### 3. 测试约定

- 测试文件命名：`test_<功能>.rs`
- 测试函数命名：`test_<场景>_<预期结果>`
- 使用 `mockito` mock HTTP 请求
- 使用 `tempfile` 创建临时测试目录

### 4. 安全考虑

- 高风险操作（bash、文件修改）需要审批
- API 密钥不记录到日志
- 敏感文件（.env、.pem）自动排除在搜索外

### 5. 性能优化

- Anthropic API 支持 prompt caching
- 大上下文自动压缩
- 使用 stream 模式减少首字延迟

---

## 典型工作流程

### 用户交互流程

```
1. 用户输入请求
   ↓
2. Agent 构建消息（包含项目概览、记忆摘要）
   ↓
3. 调用 Provider API
   ↓
4. 接收响应（流式）
   ↓
5. 如果有 tool_use：
   a. 检查风险等级
   b. 如需审批，询问用户
   c. 执行工具
   d. 将结果加入消息
   e. 返回步骤 3
   ↓
6. 检查上下文大小，必要时压缩
   ↓
7. 返回结果给用户
```

### 会话恢复流程

```
1. 检查 --continue 或 --resume 参数
   ↓
2. 从 ~/.matrix/sessions/ 加载会话
   ↓
3. 恢复消息历史
   ↓
4. 加载记忆摘要
   ↓
5. 继续对话
```

---

## 技术栈

| 类别 | 技术 |
|------|------|
| 语言 | Rust (Edition 2024) |
| 异步运行时 | Tokio |
| HTTP 客户端 | reqwest |
| 序列化 | serde, serde_json |
| CLI 框架 | clap |
| 终端 UI | rustyline, termimad, indicatif, crossterm |
| 日志 | log, env_logger |
| 测试 | mockito, tempfile, tokio-test |

---

## 版本与发布

- **当前版本**: 0.2.1
- **许可证**: MIT
- **发布渠道**: 
  - GitHub Releases（预编译二进制）
  - crates.io（cargo install）
  - npm（npm install -g @bigfishnpm/matrixcode）