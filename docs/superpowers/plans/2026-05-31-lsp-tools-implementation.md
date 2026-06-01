# LSP 工具调用能力实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 MatrixCode 添加 LSP 工具调用能力，让 AI 能实时获取代码类型信息、定义位置、引用和诊断。

**Architecture:** 新建 LSP Transport（JSON-RPC over stdio）与 LSP 服务器通信，LspClient 管理单个服务器连接，LspClientRegistry 管理多语言客户端，4 个工具通过 Registry 获取对应语言的客户端执行查询。

**Tech Stack:** Rust, lsp-types crate, tokio async runtime, JSON-RPC 2.0

---

## 文件结构

```
packages/core/
├── Cargo.toml                     # 添加 lsp-types 依赖
└── src/
    ├── lsp/
    │   ├── mod.rs                 # 更新：re-export 新模块
    │   ├── transport.rs           # 新增：LSP 传输层
    │   ├── client.rs              # 新增：LSP 客户端
    │   ├── registry.rs            # 新增：多客户端管理
    │   └── tools.rs               # 新增：LSP 工具定义
    │   ├── types.rs               # 保持不变
    │   └── manager.rs             # 保持不变
    └── tools/
        └── mod.rs                 # 更新：集成 LSP 工具

packages/cli/
└── src/
    └── terminal/
        ├── agent.rs               # 更新：传递 LspClientRegistry
        └── lsp_handler.rs         # 更新：启动实际客户端
```

---

### Task 1: 添加 lsp-types 依赖

**Files:**
- Modify: `packages/core/Cargo.toml`

- [ ] **Step 1: 添加 lsp-types 依赖到 Cargo.toml**

在 `packages/core/Cargo.toml` 的 `[dependencies]` 部分添加：

```toml
lsp-types = "0.95"
```

- [ ] **Step 2: 运行 cargo check 验证依赖下载**

Run: `cd packages/core && cargo check`
Expected: 成功下载 lsp-types crate

- [ ] **Step 3: Commit**

```bash
git add packages/core/Cargo.toml
git commit -m "feat(core): add lsp-types dependency for LSP tools"
```

---

### Task 2: 实现 LSP Transport

**Files:**
- Create: `packages/core/src/lsp/transport.rs`

- [ ] **Step 1: 创建 LSP Transport 文件**

创建 `packages/core/src/lsp/transport.rs`：

```rust
//! LSP Transport Layer
//!
//! 通过 stdio 与 LSP 服务器进程通信，处理 JSON-RPC 2.0 消息格式。

use anyhow::{Result, anyhow};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

/// LSP 传输层
pub struct LspTransport {
    /// 子进程
    process: Arc<Mutex<Option<Child>>>,
    /// 写入端 (进程 stdin)
    stdin: Arc<Mutex<Option<Box<dyn AsyncWrite + Unpin + Send>>>>,
    /// 读取端 (进程 stdout)
    stdout_reader: Arc<Mutex<Option<BufReader<Box<dyn AsyncRead + Unpin + Send>>>>>,
    /// 请求 ID 计数器
    request_id: AtomicU32,
    /// 服务器名称（用于日志）
    server_name: String,
}

impl LspTransport {
    /// 启动 LSP 服务器进程
    pub async fn spawn(
        server_name: impl Into<String>,
        command: &str,
        args: &[String],
    ) -> Result<Self> {
        let server_name = server_name.into();

        // Windows 兼容性处理
        let (actual_command, actual_args) = if cfg!(target_os = "windows")
            && (command == "npx" || command == "npm" || command == "node")
        {
            let mut full_args = vec!["/c".to_string(), command.to_string()];
            full_args.extend(args.iter().cloned());
            ("cmd.exe".to_string(), full_args)
        } else {
            (command.to_string(), args.to_vec())
        };

        let mut cmd = Command::new(&actual_command);
        cmd.args(&actual_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            anyhow!(
                "Failed to spawn LSP server '{}': {} (command: {} {:?})",
                server_name,
                e,
                actual_command,
                actual_args
            )
        })?;

        let stdin = child.stdin.take().map(|s| Box::new(s) as Box<dyn AsyncWrite + Unpin + Send>);
        let stdout = child.stdout.take().map(|s| {
            Box::new(s) as Box<dyn AsyncRead + Unpin + Send>
        });
        
        let stdout_reader = stdout.map(|s| BufReader::new(s));

        log::info!(
            "LSP server '{}' spawned successfully (pid: {:?})",
            server_name,
            child.id()
        );

        Ok(Self {
            process: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout_reader: Arc::new(Mutex::new(stdout_reader)),
            request_id: AtomicU32::new(1),
            server_name,
        })
    }

    /// 发送请求并等待响应
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.send_message(&message.to_string()).await?;
        self.receive_response(id).await
    }

    /// 发送通知（无需响应）
    pub async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(&message.to_string()).await
    }

    /// 发送原始消息（带 Content-Length header）
    async fn send_message(&self, content: &str) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        let stdin = stdin.as_mut().ok_or_else(|| anyhow!("stdin not available"))?;

        let header = format!("Content-Length: {}\r\n\r\n", content.len());
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(content.as_bytes()).await?;
        stdin.flush().await?;

        log::debug!("LSP '{}' sent: {}", self.server_name, content);
        Ok(())
    }

    /// 接收响应
    async fn receive_response(&self, expected_id: u32) -> Result<serde_json::Value> {
        let timeout_duration = Duration::from_secs(30);
        
        timeout(timeout_duration, async {
            loop {
                let message = self.receive_message().await?;
                
                // 解析消息
                let json: serde_json::Value = serde_json::from_str(&message)?;
                
                // 检查是否是我们要的响应
                if let Some(id) = json.get("id").and_then(|i| i.as_u64()) {
                    if id == expected_id as u64 {
                        // 检查是否有错误
                        if let Some(error) = json.get("error") {
                            return Err(anyhow!("LSP error: {:?}", error));
                        }
                        return Ok(json.get("result").cloned().unwrap_or(serde_json::Value::Null));
                    }
                }
                
                // 不是我们要的响应，继续等待（可能是 notification）
                log::debug!("LSP '{}' received other message: {}", self.server_name, message);
            }
        }).await.map_err(|_| anyhow!("LSP request timeout after {}s", timeout_duration.as_secs()))?
    }

    /// 接收一条消息
    pub async fn receive_message(&self) -> Result<String> {
        let mut reader = self.stdout_reader.lock().await;
        let reader = reader.as_mut().ok_or_else(|| anyhow!("stdout not available"))?;

        // 读取 Content-Length header
        let mut header_line = String::new();
        reader.read_line(&mut header_line).await?;
        
        // 解析 Content-Length
        let content_length: usize = header_line
            .strip_prefix("Content-Length: ")
            .and_then(|s| s.trim().parse().ok())
            .ok_or_else(|| anyhow!("Invalid LSP header: {}", header_line))?;

        // 读取空行
        let mut empty_line = String::new();
        reader.read_line(&mut empty_line).await?;

        // 读取内容
        let mut content = vec![0u8; content_length];
        reader.read_exact(&mut content).await?;
        
        let message = String::from_utf8(content)?;
        log::debug!("LSP '{}' received: {}", self.server_name, message);
        
        Ok(message)
    }

    /// 关闭连接
    pub async fn close(&self) -> Result<()> {
        let mut process = self.process.lock().await;
        if let Some(mut child) = process.take() {
            child.kill().await?;
            log::info!("LSP server '{}' stopped", self.server_name);
        }
        Ok(())
    }

    /// 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}
```

- [ ] **Step 2: 运行 cargo check 验证编译**

Run: `cd packages/core && cargo check 2>&1 | tail -20`
Expected: 编译成功（可能有 unused warnings）

- [ ] **Step 3: Commit**

```bash
git add packages/core/src/lsp/transport.rs
git commit -m "feat(core): add LSP transport layer for JSON-RPC communication"
```

---

### Task 3: 实现 LSP Client

**Files:**
- Create: `packages/core/src/lsp/client.rs`

- [ ] **Step 1: 创建 LSP Client 文件**

创建 `packages/core/src/lsp/client.rs`：

```rust
//! LSP Client
//!
//! 管理单个 LSP 服务器连接，提供高级 API。

use anyhow::{Result, anyhow};
use lsp_types::{
    ClientCapabilities, InitializeParams, InitializedParams,
    TextDocumentIdentifier, TextDocumentPositionParams,
    HoverParams, HoverRequest, GotoDefinitionParams, GotoDefinitionRequest,
    ReferenceParams, FindLocationsRequest, Location, Diagnostic,
    Url, Position, TextDocumentItem,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::transport::LspTransport;
use super::types::LspServerConfig;

/// LSP 客户端
pub struct LspClient {
    transport: Arc<LspTransport>,
    language: String,
    server_name: String,
    project_root: PathBuf,
    /// 已打开的文件（用于缓存）
    open_files: Arc<Mutex<Vec<PathBuf>>>,
    /// 诊断信息缓存
    diagnostics_cache: Arc<Mutex<HashMap<PathBuf, Vec<Diagnostic>>>,
}

use std::collections::HashMap;

impl LspClient {
    /// 启动并初始化 LSP 服务器
    pub async fn spawn(config: &LspServerConfig, project_root: &Path) -> Result<Self> {
        let transport = LspTransport::spawn(
            &config.language,
            &config.command,
            &config.args,
        ).await?;

        let client = Self {
            transport: Arc::new(transport),
            language: config.language.clone(),
            server_name: config.command.clone(),
            project_root: project_root.to_path_buf(),
            open_files: Arc::new(Mutex::new(Vec::new())),
            diagnostics_cache: Arc::new(Mutex::new(HashMap::new())),
        };

        // 发送 initialize 请求
        client.initialize().await?;

        // 发送 initialized 通知
        client.initialized().await?;

        log::info!(
            "LSP client '{}' initialized for language '{}'",
            client.server_name,
            client.language
        );

        Ok(client)
    }

    /// 初始化握手
    async fn initialize(&self) -> Result<()> {
        let root_uri = Url::from_file_path(&self.project_root)
            .map_err(|_| anyhow!("Invalid project root path"))?;

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };

        let result = self.transport.send_request(
            "initialize",
            serde_json::to_value(params)?,
        ).await?;

        log::debug!("LSP '{}' initialize result: {:?}", self.server_name, result);
        Ok(())
    }

    /// 发送 initialized 通知
    async fn initialized(&self) -> Result<()> {
        self.transport.send_notification(
            "initialized",
            serde_json::to_value(InitializedParams {})?,
        ).await
    }

    /// 打开文件
    pub async fn open_file(&self, path: &Path, content: &str) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow!("Invalid file path"))?;

        let text_document = TextDocumentItem {
            uri,
            language_id: self.language.clone(),
            version: 0,
            text: content.to_string(),
        };

        self.transport.send_notification(
            "textDocument/didOpen",
            serde_json::json!({ "textDocument": text_document }),
        ).await?;

        self.open_files.lock().await.push(path.to_path_buf());
        log::debug!("LSP '{}' opened file: {}", self.server_name, path.display());
        Ok(())
    }

    /// 获取悬停信息
    pub async fn hover(&self, path: &Path, line: u32, column: u32) -> Result<Option<HoverResult>> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow!("Invalid file path"))?;

        let params = HoverParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position { line, character: column },
        };

        let result = self.transport.send_request(
            "textDocument/hover",
            serde_json::to_value(params)?,
        ).await?;

        // 解析结果
        if result.is_null() {
            return Ok(None);
        }

        let hover: lsp_types::Hover = serde_json::from_value(result)?;
        Ok(Some(HoverResult::from_lsp_hover(hover)))
    }

    /// 获取定义位置
    pub async fn definition(&self, path: &Path, line: u32, column: u32) -> Result<Vec<Location>> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow!("Invalid file path"))?;

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character: column },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self.transport.send_request(
            "textDocument/definition",
            serde_json::to_value(params)?,
        ).await?;

        // 解析结果
        if result.is_null() {
            return Ok(Vec::new());
        }

        // GotoDefinitionResponse 可能是 Location 或 LocationLink
        let locations = parse_definition_response(result)?;
        Ok(locations)
    }

    /// 获取所有引用
    pub async fn references(&self, path: &Path, line: u32, column: u32) -> Result<Vec<Location>> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow!("Invalid file path"))?;

        let params = ReferenceParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character: column },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        let result = self.transport.send_request(
            "textDocument/references",
            serde_json::to_value(params)?,
        ).await?;

        // 解析结果
        if result.is_null() {
            return Ok(Vec::new());
        }

        let locations: Vec<Location> = serde_json::from_value(result)?;
        Ok(locations)
    }

    /// 获取诊断信息（从缓存读取）
    pub async fn diagnostics(&self, path: &Path) -> Result<Vec<Diagnostic>> {
        let cache = self.diagnostics_cache.lock().await;
        let diagnostics = cache.get(path).cloned().unwrap_or_default();
        Ok(diagnostics)
    }

    /// 更新诊断缓存（收到 publishDiagnostics 时调用）
    pub async fn update_diagnostics(&self, path: &Path, diagnostics: Vec<Diagnostic>) {
        let mut cache = self.diagnostics_cache.lock().await;
        cache.insert(path.to_path_buf(), diagnostics);
    }

    /// 关闭连接
    pub async fn shutdown(&self) -> Result<()> {
        self.transport.send_request("shutdown", serde_json::Value::Null).await?;
        self.transport.send_notification("exit", serde_json::Value::Null).await?;
        self.transport.close().await
    }

    /// 获取语言标识
    pub fn language(&self) -> &str {
        &self.language
    }

    /// 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

/// 悬停结果（人类可读格式）
#[derive(Debug, Clone)]
pub struct HoverResult {
    /// 类型签名
    pub signature: Option<String>,
    /// 文档内容
    pub documentation: Option<String>,
}

impl HoverResult {
    fn from_lsp_hover(hover: lsp_types::Hover) -> Self {
        let signature = hover.contents.as_string().cloned();
        
        let documentation = match &hover.contents {
            lsp_types::HoverContents::MarkupContent(markup) => Some(markup.value.clone()),
            lsp_types::HoverContents::Array(arr) => {
                arr.iter()
                    .filter_map(|c| c.as_string().cloned())
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into()
            }
            _ => None,
        };

        Self { signature, documentation }
    }

    /// 格式化为人类可读文本
    pub fn to_human_readable(&self) -> String {
        let mut parts = Vec::new();

        if let Some(sig) = &self.signature {
            parts.push(format!("【类型】{}", sig));
        }

        if let Some(doc) = &self.documentation {
            parts.push(format!("【文档】{}", doc));
        }

        if parts.is_empty() {
            "无悬停信息".to_string()
        } else {
            parts.join("\n")
        }
    }
}

/// 解析定义响应
fn parse_definition_response(result: serde_json::Value) -> Result<Vec<Location>> {
    // 尝试解析为 Location 数组
    if let Ok(locations) = serde_json::from_value::<Vec<Location>>(result.clone()) {
        return Ok(locations);
    }

    // 尝试解析为单个 Location
    if let Ok(location) = serde_json::from_value::<Location>(result.clone()) {
        return Ok(vec![location]);
    }

    // 尝试解析为 LocationLink 数组
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(result.clone()) {
        return Ok(links.into_iter().map(|link| Location {
            uri: link.target_uri,
            range: link.target_selection_range,
        }).collect());
    }

    // 无法解析，返回空
    Ok(Vec::new())
}

/// 格式化 Location 为人类可读文本
pub fn format_location(location: &Location) -> String {
    let path = location.uri.to_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| location.uri.to_string());
    
    format!(
        "{}:{}:{}",
        path,
        location.range.start.line + 1,
        location.range.start.character + 1
    )
}

/// 格式化 Diagnostic 为人类可读文本
pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let severity = match diagnostic.severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => "❌ 错误",
        Some(lsp_types::DiagnosticSeverity::WARNING) => "⚠️ 警告",
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => "ℹ️ 信息",
        Some(lsp_types::DiagnosticSeverity::HINT) => "💡 提示",
        _ => "📝",
    };

    let code = diagnostic.code.as_ref()
        .and_then(|c| match c {
            lsp_types::NumberOrString::Number(n) => Some(format!("[E{}]", n)),
            lsp_types::NumberOrString::String(s) => Some(format!("[{}]", s)),
        })
        .unwrap_or_default();

    let position = format!(
        "{}:{}",
        diagnostic.range.start.line + 1,
        diagnostic.range.start.character + 1
    );

    format!(
        "{} {} {}\n   位置: {}\n   {}",
        severity,
        code,
        diagnostic.message,
        position,
        diagnostic.related_information.as_ref()
            .map(|info| info.iter()
                .map(|i| i.message.clone())
                .collect::<Vec<_>>()
                .join("; "))
            .unwrap_or_default()
    )
}
```

- [ ] **Step 2: 运行 cargo check 验证编译**

Run: `cd packages/core && cargo check 2>&1 | tail -20`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add packages/core/src/lsp/client.rs
git commit -m "feat(core): add LSP client with hover, definition, references support"
```

---

### Task 4: 实现 LspClientRegistry

**Files:**
- Create: `packages/core/src/lsp/registry.rs`

- [ ] **Step 1: 创建 LspClientRegistry 文件**

创建 `packages/core/src/lsp/registry.rs`：

```rust
//! LSP Client Registry
//!
//! 管理多个语言的 LSP 客户端。

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::client::LspClient;
use super::types::LspServerConfig;

/// LSP 客户端注册表
pub struct LspClientRegistry {
    /// 语言 -> 客户端映射
    clients: Arc<RwLock<HashMap<String, Arc<LspClient>>>>,
}

impl LspClientRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 启动并注册 LSP 客户端
    pub async fn register(&self, config: &LspServerConfig, project_root: &Path) -> Result<()> {
        let client = LspClient::spawn(config, project_root).await?;
        let mut clients = self.clients.write().await;
        clients.insert(config.language.clone(), Arc::new(client));
        log::info!("LSP client registered for language: {}", config.language);
        Ok(())
    }

    /// 获取指定语言的客户端
    pub async fn get_client(&self, language: &str) -> Option<Arc<LspClient>> {
        let clients = self.clients.read().await;
        clients.get(language).cloned()
    }

    /// 是否有活跃客户端
    pub async fn has_active_clients(&self) -> bool {
        let clients = self.clients.read().await;
        !clients.is_empty()
    }

    /// 获取所有活跃语言
    pub async fn active_languages(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// 关闭所有客户端
    pub async fn shutdown_all(&self) -> Result<()> {
        let clients = self.clients.write().await;
        for (language, client) in clients.iter() {
            if let Err(e) = client.shutdown().await {
                log::warn!("Failed to shutdown LSP client '{}': {}", language, e);
            }
        }
        clients.clear();
        Ok(())
    }
}

impl Default for LspClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: 运行 cargo check**

Run: `cd packages/core && cargo check 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add packages/core/src/lsp/registry.rs
git commit -m "feat(core): add LspClientRegistry for multi-language client management"
```

---

### Task 5: 实现 LSP 工具

**Files:**
- Create: `packages/core/src/lsp/tools.rs`

- [ ] **Step 1: 创建 LSP 工具文件**

创建 `packages/core/src/lsp/tools.rs`：

```rust
//! LSP Tools
//!
//! 提供给 AI 调用的 LSP 工具定义。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use super::registry::LspClientRegistry;
use super::client::{format_location, format_diagnostic};

/// LSP Hover 工具
pub struct LspHoverTool {
    registry: Arc<LspClientRegistry>,
}

impl LspHoverTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for LspHoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_hover".to_string(),
            description: "获取符号的类型信息和文档。返回类型签名、文档注释等。\n\n参数:\n- file: 文件路径（绝对路径）\n- line: 行号（从 0 开始）\n- column: 列号（从 0 开始）\n\n适用场景:\n- 查看函数/变量的类型签名\n- 获取 API 文档说明\n- 了解参数和返回值类型".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件绝对路径"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（从 0 开始）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（从 0 开始）"
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file: String = params["file"].as_str()
            .ok_or_else(|| anyhow!("Missing 'file' parameter"))?
            .to_string();
        let line: u32 = params["line"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'line' parameter"))? as u32;
        let column: u32 = params["column"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'column' parameter"))? as u32;

        let path = PathBuf::from(&file);
        
        // 根据文件扩展名确定语言
        let language = detect_language_from_path(&path);
        
        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("No LSP client available for language: {}", language))?;

        // 先打开文件
        let content = std::fs::read_to_string(&path)?;
        client.open_file(&path, &content).await?;

        let result = client.hover(&path, line, column).await?;

        match result {
            Some(hover) => Ok(format!(
                "{}\n【来源】{}",
                hover.to_human_readable(),
                client.server_name()
            )),
            None => Ok("未找到悬停信息".to_string()),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// LSP Definition 工具
pub struct LspDefinitionTool {
    registry: Arc<LspClientRegistry>,
}

impl LspDefinitionTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for LspDefinitionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_definition".to_string(),
            description: "跳转到符号的定义位置。返回定义所在的文件和位置。\n\n参数:\n- file: 文件路径（绝对路径）\n- line: 行号（从 0 开始）\n- column: 列号（从 0 开始）\n\n适用场景:\n- 查找函数的定义位置\n- 查看变量在哪里声明\n- 跳转到类型定义".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件绝对路径"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（从 0 开始）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（从 0 开始）"
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file: String = params["file"].as_str()
            .ok_or_else(|| anyhow!("Missing 'file' parameter"))?
            .to_string();
        let line: u32 = params["line"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'line' parameter"))? as u32;
        let column: u32 = params["column"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'column' parameter"))? as u32;

        let path = PathBuf::from(&file);
        let language = detect_language_from_path(&path);
        
        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("No LSP client available for language: {}", language))?;

        let content = std::fs::read_to_string(&path)?;
        client.open_file(&path, &content).await?;

        let locations = client.definition(&path, line, column).await?;

        if locations.is_empty() {
            return Ok("未找到定义位置".to_string());
        }

        let result = locations.iter()
            .map(|l| format!("定义位于: {}", format_location(l)))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// LSP References 工具
pub struct LspReferencesTool {
    registry: Arc<LspClientRegistry>,
}

impl LspReferencesTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for LspReferencesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_references".to_string(),
            description: "查找符号的所有引用位置。\n\n参数:\n- file: 文件路径（绝对路径）\n- line: 行号（从 0 开始）\n- column: 列号（从 0 开始）\n\n适用场景:\n- 查看函数被哪些地方调用\n- 找到变量的所有使用位置\n- 分析代码依赖关系".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件绝对路径"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（从 0 开始）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（从 0 开始）"
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file: String = params["file"].as_str()
            .ok_or_else(|| anyhow!("Missing 'file' parameter"))?
            .to_string();
        let line: u32 = params["line"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'line' parameter"))? as u32;
        let column: u32 = params["column"].as_u64()
            .ok_or_else(|| anyhow!("Missing 'column' parameter"))? as u32;

        let path = PathBuf::from(&file);
        let language = detect_language_from_path(&path);
        
        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("No LSP client available for language: {}", language))?;

        let content = std::fs::read_to_string(&path)?;
        client.open_file(&path, &content).await?;

        let locations = client.references(&path, line, column).await?;

        if locations.is_empty() {
            return Ok("未找到引用".to_string());
        }

        let result = format!(
            "找到 {} 个引用:\n{}",
            locations.len(),
            locations.iter()
                .enumerate()
                .map(|(i, l)| format!("{}. {}", i + 1, format_location(l)))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// LSP Diagnostics 工具
pub struct LspDiagnosticsTool {
    registry: Arc<LspClientRegistry>,
}

impl LspDiagnosticsTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for LspDiagnosticsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_diagnostics".to_string(),
            description: "获取文件的诊断信息（错误、警告）。\n\n参数:\n- file: 文件路径（绝对路径）\n\n适用场景:\n- 查看文件是否有编译错误\n- 获取类型检查警告\n- 诊断代码问题".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件绝对路径"
                    }
                },
                "required": ["file"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file: String = params["file"].as_str()
            .ok_or_else(|| anyhow!("Missing 'file' parameter"))?
            .to_string();

        let path = PathBuf::from(&file);
        let language = detect_language_from_path(&path);
        
        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("No LSP client available for language: {}", language))?;

        let content = std::fs::read_to_string(&path)?;
        client.open_file(&path, &content).await?;

        let diagnostics = client.diagnostics(&path).await?;

        if diagnostics.is_empty() {
            return Ok(format!("诊断结果 ({}):\n✅ 无错误或警告", file));
        }

        let result = format!(
            "诊断结果 ({}):\n{}",
            file,
            diagnostics.iter()
                .map(|d| format_diagnostic(d))
                .collect::<Vec<_>>()
                .join("\n\n")
        );

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// 根据文件路径检测语言
fn detect_language_from_path(path: &PathBuf) -> String {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => "rust",
        "go" => "go",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        _ => ext.to_string(),
    }
}

/// 创建所有 LSP 工具
pub fn lsp_tools(registry: Arc<LspClientRegistry>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(LspHoverTool::new(registry.clone())),
        Box::new(LspDefinitionTool::new(registry.clone())),
        Box::new(LspReferencesTool::new(registry.clone())),
        Box::new(LspDiagnosticsTool::new(registry)),
    ]
}
```

- [ ] **Step 2: 运行 cargo check**

Run: `cd packages/core && cargo check 2>&1 | tail -20`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add packages/core/src/lsp/tools.rs
git commit -m "feat(core): add LSP tools: hover, definition, references, diagnostics"
```

---

### Task 6: 更新 lsp/mod.rs 导出新模块

**Files:**
- Modify: `packages/core/src/lsp/mod.rs`

- [ ] **Step 1: 更新 lsp/mod.rs 添加新模块导出**

修改 `packages/core/src/lsp/mod.rs`，在现有内容基础上添加：

```rust
pub mod transport;
pub mod client;
pub mod registry;
pub mod tools;

// Re-export new types
pub use transport::LspTransport;
pub use client::{LspClient, HoverResult};
pub use registry::LspClientRegistry;
pub use tools::{lsp_tools, LspHoverTool, LspDefinitionTool, LspReferencesTool, LspDiagnosticsTool};
```

- [ ] **Step 2: 运行 cargo check**

Run: `cd packages/core && cargo check 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add packages/core/src/lsp/mod.rs
git commit -m "feat(core): export new LSP modules in mod.rs"
```

---

### Task 7: 集成 LSP 工具到工具系统

**Files:**
- Modify: `packages/core/src/tools/mod.rs`

- [ ] **Step 1: 修改 all_tools_full 函数添加 LSP 工具参数**

修改 `packages/core/src/tools/mod.rs` 中的 `all_tools_full` 函数：

```rust
/// Build full toolset with provider and project path.
pub fn all_tools_full(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
    project_path: PathBuf,
    lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,  // 新增参数
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    
    // CodeGraph tools
    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }
    
    // LSP tools - 只在有活跃客户端时注入
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }
    
    // Workflow tools
    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}
```

- [ ] **Step 2: 同步修改其他相关函数签名**

同样修改 `all_tools_with_project_path`：

```rust
pub fn all_tools_with_project_path(
    skills: Arc<Vec<Skill>>,
    project_path: PathBuf,
    lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }
    tools.extend(workflow::workflow_tools());
    tools
}
```

- [ ] **Step 3: 运行 cargo check**

Run: `cd packages/core && cargo check 2>&1 | tail -20`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add packages/core/src/tools/mod.rs
git commit -m "feat(core): integrate LSP tools into tool system"
```

---

### Task 8: 更新 Agent 启动流程集成 LSP

**Files:**
- Modify: `packages/cli/src/terminal/lsp_handler.rs`
- Modify: `packages/cli/src/terminal/agent.rs`

- [ ] **Step 1: 更新 lsp_handler.rs 启动实际客户端**

修改 `packages/cli/src/terminal/lsp_handler.rs`：

```rust
//! LSP Handler
//!
//! Handles LSP server startup, status, and lifecycle management.

use std::sync::Arc;
use matrixcode_core::{AgentEvent, lsp::{LspManager, LspServerInfo, LspClientRegistry, LspServerConfig}};
use std::path::PathBuf;

/// LSP manager that handles server lifecycle
pub struct LspHandler {
    manager: Arc<tokio::sync::RwLock<LspManager>>,
    registry: Arc<LspClientRegistry>,
}

impl LspHandler {
    /// Create new LSP handler
    pub fn new() -> Self {
        Self {
            manager: Arc::new(tokio::sync::RwLock::new(LspManager::new())),
            registry: Arc::new(LspClientRegistry::new()),
        }
    }

    /// Add servers from config and spawn actual clients
    pub async fn add_servers(&self, lsp_servers: Vec<(String, LspServerConfig)>, project_root: PathBuf) {
        let mut manager = self.manager.write().await;
        for (name, config) in lsp_servers {
            manager.add_server(config.clone());
            log::info!("LSP server '{}' added to manager", name);
            
            // 启动实际客户端
            if let Err(e) = self.registry.register(&config, &project_root).await {
                log::warn!("Failed to start LSP client '{}': {}", name, e);
                manager.mark_error(&config.language, e.to_string());
            } else {
                log::info!("LSP client '{}' started successfully", name);
            }
        }
    }

    /// Start all LSP servers and notify UI
    pub async fn start_all(&self, event_tx: &tokio::sync::mpsc::Sender<AgentEvent>) {
        let manager = self.manager.write().await;

        let servers: Vec<_> = manager.server_infos();
        
        for server in &servers {
            manager.mark_connected(&server.language);
        }

        let servers = manager.server_infos();

        for server in &servers {
            let _ = event_tx.send(AgentEvent::lsp_server_added(
                server.name.clone(),
                server.language.clone(),
            )).await;
        }

        let _ = event_tx.send(AgentEvent::lsp_server_status(servers)).await;
    }

    /// Get LSP registry for tool injection
    pub fn registry(&self) -> Arc<LspClientRegistry> {
        self.registry.clone()
    }

    /// Get server statuses
    #[allow(dead_code)]
    pub async fn get_status(&self) -> Vec<LspServerInfo> {
        self.manager.read().await.server_infos()
    }
}

impl Default for LspHandler {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: 更新 agent.rs 传递 LspClientRegistry**

修改 `packages/cli/src/terminal/agent.rs`，在 `AgentContext` 中添加 `lsp_registry`：

```rust
pub struct AgentContext {
    // ... 现有字段 ...
    pub lsp_servers: Vec<(String, matrixcode_core::lsp::LspServerConfig)>,
    pub project_path: Option<PathBuf>,  // 确保有这个字段
}
```

然后在 `run_agent_task` 函数中：

```rust
pub async fn run_agent_task(ctx: AgentContext) {
    // ... 现有代码 ...
    
    // LSP Handler
    let lsp_handler = LspHandler::new();
    
    // 启动 LSP 客户端（传入 project_root）
    if let Some(project_root) = ctx.project_path.clone() {
        lsp_handler.add_servers(ctx.lsp_servers, project_root).await;
    } else {
        lsp_handler.add_servers(ctx.lsp_servers, std::env::current_dir().unwrap_or_default()).await;
    }
    
    lsp_handler.start_all(&event_tx).await;
    
    // 获取 registry 用于工具注入
    let lsp_registry = lsp_handler.registry();
    
    // 构建 Provider
    let provider = create_provider(...);
    
    // 构建工具列表（传入 lsp_registry）
    let tools = matrixcode_core::tools::all_tools_full(
        skills,
        provider,
        project_path,
        Some(lsp_registry),  // 新增参数
    );
    
    // ... 后续代码 ...
}
```

- [ ] **Step 3: 运行 cargo check**

Run: `cd packages/cli && cargo check 2>&1 | tail -30`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add packages/cli/src/terminal/lsp_handler.rs packages/cli/src/terminal/agent.rs
git commit -m "feat(cli): integrate LSP client startup into Agent flow"
```

---

### Task 9: 编写测试

**Files:**
- Create: `packages/core/tests/test_lsp.rs`

- [ ] **Step 1: 创建 LSP 测试文件**

创建 `packages/core/tests/test_lsp.rs`：

```rust
//! LSP integration tests

use matrixcode_core::lsp::{LspServerConfig, LspClientRegistry};

#[tokio::test]
async fn test_lsp_registry_creation() {
    let registry = LspClientRegistry::new();
    assert!(!registry.has_active_clients().await);
}

#[tokio::test]
async fn test_detect_language_from_extension() {
    use std::path::PathBuf;
    
    fn detect_language(path: &PathBuf) -> String {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "rs" => "rust",
            "go" => "go",
            "ts" => "typescript",
            "py" => "python",
            _ => ext.to_string(),
        }
    }

    assert_eq!(detect_language(&PathBuf::from("test.rs")), "rust");
    assert_eq!(detect_language(&PathBuf::from("test.go")), "go");
    assert_eq!(detect_language(&PathBuf::from("test.ts")), "typescript");
    assert_eq!(detect_language(&PathBuf::from("test.py")), "python");
}

#[test]
fn test_lsp_server_config_creation() {
    let config = LspServerConfig::new("rust-analyzer", "rust");
    assert_eq!(config.command, "rust-analyzer");
    assert_eq!(config.language, "rust");
    assert!(config.enabled);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd packages/core && cargo test test_lsp 2>&1`
Expected: 测试通过

- [ ] **Step 3: Commit**

```bash
git add packages/core/tests/test_lsp.rs
git commit -m "test(core): add LSP registry and config tests"
```

---

### Task 10: 最终验证

- [ ] **Step 1: 运行完整构建**

Run: `cd packages/cli && cargo build --release 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 2: 运行所有测试**

Run: `cd packages/core && cargo test 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 3: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete LSP tools implementation with hover, definition, references, diagnostics"
```

---

## 实现顺序总结

1. Task 1 - 添加 lsp-types 依赖
2. Task 2 - 实现 LSP Transport
3. Task 3 - 实现 LSP Client
4. Task 4 - 实现 LspClientRegistry
5. Task 5 - 实现 LSP 工具
6. Task 6 - 更新 mod.rs 导出
7. Task 7 - 集成到工具系统
8. Task 8 - 集成到 Agent 启动流程
9. Task 9 - 编写测试
10. Task 10 - 最终验证