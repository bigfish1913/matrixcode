# LSP 工具调用能力设计

日期: 2026-05-31
状态: 设计完成，待实现

## 概述

为 MatrixCode 添加 LSP (Language Server Protocol) 工具调用能力，让 AI 能够实时获取代码的类型信息、定义位置、引用和诊断信息。

## 功能范围

4 个核心工具：
- `lsp_hover` - 获取符号的类型信息和文档
- `lsp_definition` - 跳转到符号的定义位置
- `lsp_references` - 查找符号的所有引用
- `lsp_diagnostics` - 获取文件的诊断信息（错误/警告）

## 关键决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 启动方式 | 预启动 | Agent 启动时启动所有检测到的服务器，避免首次调用延迟 |
| 输出格式 | 人类可读文本 | 易于 AI 理解，与其他工具风格一致，节省 token |
| 通信层 | 新建 LSP Transport | LSP 用 JSON-RPC 2.0，与 MCP 协议不同 |
| 类型定义 | lsp-types crate | 标准库，避免手动定义数百个类型 |

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│  Agent 启动                                                  │
│  ├── LspHandler.add_servers()  ← 添加检测到的服务器配置       │
│  └── LspHandler.start_all()    ← 启动所有 LSP 服务器进程      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  LspClient (新增)                                           │
│  ├── spawn()        启动 LSP 服务器进程                      │
│  ├── initialize()   发送 LSP initialize 请求                │
│  ├── hover()        textDocument/hover                      │
│  ├── definition()   textDocument/definition                  │
│  ├── references()   textDocument/references                  │
│  └── diagnostics()  textDocument/publishDiagnostics (监听)   │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  LSP Tools (新增)                                           │
│  ├── lsp_hover      获取符号类型/文档                        │
│  ├── lsp_definition 跳转到定义位置                           │
│  ├── lsp_references 查找所有引用                             │
│  └── lsp_diagnostics 获取文件诊断信息                        │
└─────────────────────────────────────────────────────────────┘
```

## 文件结构

```
packages/core/src/lsp/
├── mod.rs           # 模块入口，re-export
├── types.rs         # 现有类型（保持不变）
├── manager.rs       # 现有管理器（保持不变）
├── client.rs        # 新增：LSP 客户端实现
├── transport.rs     # 新增：LSP 传输层（JSON-RPC over stdio）
├── protocol.rs      # 新增：LSP 协议辅助（或直接用 lsp-types）
└── tools.rs         # 新增：LSP 工具定义
```

## 组件详细设计

### 1. LSP Transport

职责：通过 stdio 与 LSP 服务器进程通信，处理 JSON-RPC 2.0 消息格式。

消息格式（LSP 标准）：
```
Content-Length: 123\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"textDocument/hover","params":{...}}
```

核心结构：
```rust
pub struct LspTransport {
    process: Child,
    stdin: Box<dyn AsyncWrite>,
    stdout_reader: BufReader<Box<dyn AsyncRead>>,
    request_id: AtomicU32,
}

impl LspTransport {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self>;
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value>;
    pub async fn send_notification(&self, method: &str, params: Value) -> Result<()>;
    pub async fn receive_response(&self, id: u32) -> Result<Value>;
}
```

### 2. LSP Client

职责：管理单个 LSP 服务器连接，提供高级 API。

```rust
pub struct LspClient {
    transport: LspTransport,
    language: String,
    server_name: String,
    open_files: HashMap<PathBuf, String>,
    diagnostics_cache: HashMap<PathBuf, Vec<Diagnostic>>,
}

impl LspClient {
    pub async fn spawn(config: &LspServerConfig, project_root: &Path) -> Result<Self>;
    async fn initialize(&self, project_root: &Path) -> Result<()>;
    pub async fn open_file(&self, path: &Path, content: &str) -> Result<()>;
    pub async fn hover(&self, path: &Path, line: u32, col: u32) -> Result<Option<HoverResult>>;
    pub async fn definition(&self, path: &Path, line: u32, col: u32) -> Result<Vec<Location>>;
    pub async fn references(&self, path: &Path, line: u32, col: u32) -> Result<Vec<Location>>;
    pub async fn diagnostics(&self, path: &Path) -> Result<Vec<Diagnostic>>;
    pub async fn shutdown(&self) -> Result<()>;
}
```

初始化流程：
1. spawn() 启动进程
2. initialize() 发送初始化请求，声明客户端能力
3. initialized() 发送初始化完成通知
4. 服务器进入就绪状态

### 3. LSP Tools

#### lsp_hover
```json
{
  "name": "lsp_hover",
  "description": "获取符号的类型信息和文档。返回类型签名、文档注释等。",
  "parameters": {
    "file": "文件路径（绝对路径）",
    "line": "行号（从 0 开始）",
    "column": "列号（从 0 开始）"
  }
}
```

输出示例：
```
【类型】fn foo(x: i32) -> String
【文档】这是一个示例函数...
【来源】rust-analyzer
```

#### lsp_definition
```json
{
  "name": "lsp_definition",
  "description": "跳转到符号的定义位置。返回定义所在的文件和位置。",
  "parameters": {
    "file": "文件路径",
    "line": "行号",
    "column": "列号"
  }
}
```

输出示例：
```
定义位于: src/lib.rs:42:5
类型: 函数定义
```

#### lsp_references
```json
{
  "name": "lsp_references",
  "description": "查找符号的所有引用位置。",
  "parameters": {
    "file": "文件路径",
    "line": "行号",
    "column": "列号"
  }
}
```

输出示例：
```
找到 3 个引用:
1. src/main.rs:15:8 - 调用
2. src/lib.rs:50:12 - 测试
3. tests/test.rs:20:5 - 测试
```

#### lsp_diagnostics
```json
{
  "name": "lsp_diagnostics",
  "description": "获取文件的诊断信息（错误、警告）。",
  "parameters": {
    "file": "文件路径"
  }
}
```

输出示例：
```
诊断结果 (src/main.rs):
❌ 错误 [E0425]: 未找到值 `foo`
   位置: 10:5

⚠️ 警告: 未使用的变量 `x`
   位置: 15:8
```

### 4. 工具集成

修改 `packages/core/src/tools/mod.rs`：

```rust
pub fn all_tools_full(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn Provider>,
    project_path: PathBuf,
    lsp_clients: Option<Arc<LspClientRegistry>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);

    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }

    // LSP tools - 只在有活跃 LSP 客户端时注入
    if let Some(clients) = lsp_clients {
        if clients.has_active_clients() {
            tools.extend(lsp::tools::lsp_tools(clients));
        }
    }

    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}
```

LspClientRegistry：
```rust
pub struct LspClientRegistry {
    clients: HashMap<String, Arc<LspClient>>,
}

impl LspClientRegistry {
    pub fn get_client(&self, language: &str) -> Option<&Arc<LspClient>>;
    pub fn has_active_clients(&self) -> bool;
}
```

### 5. 依赖项

新增 Cargo 依赖（packages/core/Cargo.toml）：

```toml
[dependencies]
lsp-types = "0.95"
```

lsp-types crate 提供：
- InitializeParams, InitializeResult
- HoverParams, HoverResult
- GotoDefinitionParams, GotoDefinitionResult
- ReferenceParams, Location
- Diagnostic, PublishDiagnosticsParams
- 等数百个标准 LSP 类型

### 6. 错误处理

| 场景 | 处理方式 |
|------|----------|
| LSP 服务器未安装 | 启动时检测，跳过该服务器，状态显示 Error |
| LSP 服务器启动失败 | 返回错误信息，工具调用时提示"服务器未就绪" |
| 文件不在项目根目录 | 拒绝请求，提示"文件路径不在项目范围内" |
| 服务器返回空结果 | 返回"未找到相关信息"而非错误 |
| 服务器超时（>5秒） | 返回"请求超时，服务器可能繁忙" |
| 多个 LSP 服务器同语言 | 选择第一个就绪的服务器 |

错误返回格式：
```
⚠️ LSP 服务暂时不可用: rust-analyzer 未就绪
建议: 使用 code_search 或 grep 作为替代
```

## 实现顺序

1. 添加 lsp-types 依赖
2. 实现 LspTransport（传输层）
3. 实现 LspClient（客户端）
4. 实现 LspClientRegistry（多客户端管理）
5. 实现 4 个 LSP 工具
6. 修改工具注册逻辑
7. 修改 Agent 启动流程，集成 LSP 预启动
8. 测试和调试