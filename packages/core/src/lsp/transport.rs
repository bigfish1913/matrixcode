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
                let message = self.receive_messages().await?;

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
    pub async fn receive_messages(&self) -> Result<String> {
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