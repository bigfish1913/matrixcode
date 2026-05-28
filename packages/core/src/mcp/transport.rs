//! MCP Transport Layer
//!
//! 提供两种传输方式：
//! - StdioTransport: 通过 stdin/stdout 与子进程通信（最常用）
//! - SseTransport: 通过 HTTP SSE 连接远程服务器

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

// ============================================================================
// Transport Trait
// ============================================================================

/// MCP 传输层抽象
#[async_trait]
pub trait Transport: Send + Sync {
    /// 发送请求并等待响应
    async fn send(&self, message: &str) -> Result<String>;
    
    /// 发送通知（无需响应）
    async fn notify(&self, message: &str) -> Result<()>;
    
    /// 接收一条消息
    async fn receive(&self) -> Result<String>;
    
    /// 关闭连接
    async fn close(&self) -> Result<()>;
}

// ============================================================================
// Stdio Transport
// ============================================================================

/// Stdio 传输 - 通过子进程的 stdin/stdout 通信
pub struct StdioTransport {
    /// 子进程
    process: Arc<Mutex<Option<Child>>>,
    /// 写入端 (进程 stdin)
    writer: Arc<Mutex<Option<Box<dyn AsyncWrite + Unpin + Send>>>>,
    /// 读取端 (进程 stdout)
    reader: Arc<Mutex<Option<BufReader<Box<dyn AsyncRead + Unpin + Send>>>>>,
    /// 服务器名称（用于日志）
    server_name: String,
}

impl StdioTransport {
    /// 启动 MCP 服务器进程
    pub async fn spawn(
        name: impl Into<String>,
        command: &str,
        args: &[String],
        env: Option<Vec<(String, String)>>,
    ) -> Result<Self> {
        let server_name = name.into();
        
        // Windows 兼容性：npx, npm 等需要通过 cmd.exe 运行
        let (actual_command, actual_args) = if cfg!(target_os = "windows") 
            && (command == "npx" || command == "npm" || command == "node") {
            let mut full_args = vec!["/c".to_string(), command.to_string()];
            full_args.extend(args.iter().cloned());
            ("cmd.exe".to_string(), full_args)
        } else {
            (command.to_string(), args.to_vec())
        };
        
        // 使用 tokio 异步 Command
        let mut cmd = Command::new(&actual_command);
        cmd.args(&actual_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true); // 确保进程在 drop 时被杀死
        
        // 设置环境变量
        if let Some(env_vars) = env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }
        
        // 启动进程
        let mut child = cmd.spawn()
            .map_err(|e| anyhow!("Failed to spawn MCP server '{}': {} (command: {} {:?})", 
                server_name, e, actual_command, actual_args))?;
        
        // 获取 stdin/stdout (tokio 异步版本)
        let stdin: Box<dyn AsyncWrite + Unpin + Send> = Box::new(child.stdin.take()
            .ok_or_else(|| anyhow!("Failed to get stdin for MCP server '{}'", server_name))?);
        let stdout: Box<dyn AsyncRead + Unpin + Send> = Box::new(child.stdout.take()
            .ok_or_else(|| anyhow!("Failed to get stdout for MCP server '{}'", server_name))?);
        
        tracing::info!("MCP server '{}' started: {} {:?}", server_name, actual_command, actual_args);
        
        Ok(Self {
            process: Arc::new(Mutex::new(Some(child))),
            writer: Arc::new(Mutex::new(Some(stdin))),
            reader: Arc::new(Mutex::new(Some(BufReader::new(stdout)))),
            server_name,
        })
    }
    
    /// 读取一行响应（带超时）
    async fn read_line(&self) -> Result<String> {
        let mut reader_lock = self.reader.lock().await;
        let reader = reader_lock.as_mut()
            .ok_or_else(|| anyhow!("Transport closed for server '{}'", self.server_name))?;
        
        let mut line = String::new();
        
        // 添加 30 秒超时
        let read_result = tokio::time::timeout(
            Duration::from_secs(30),
            reader.read_line(&mut line)
        ).await;
        
        match read_result {
            Ok(Ok(_)) => {
                if line.is_empty() {
                    return Err(anyhow!("EOF reached for server '{}'", self.server_name));
                }
                // 移除换行符
                Ok(line.trim_end().to_string())
            }
            Ok(Err(e)) => Err(anyhow!("Read error for server '{}': {}", self.server_name, e)),
            Err(_) => Err(anyhow!("Read timeout for server '{}' after 30s", self.server_name)),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&self, message: &str) -> Result<String> {
        let mut writer_lock = self.writer.lock().await;
        let writer = writer_lock.as_mut()
            .ok_or_else(|| anyhow!("Transport closed for server '{}'", self.server_name))?;
        
        // 发送请求（带换行符）
        writer.write_all(format!("{}\n", message).as_bytes()).await?;
        writer.flush().await?;
        
        // 等待响应
        let response = self.read_line().await?;
        Ok(response)
    }
    
    async fn notify(&self, message: &str) -> Result<()> {
        let mut writer_lock = self.writer.lock().await;
        let writer = writer_lock.as_mut()
            .ok_or_else(|| anyhow!("Transport closed for server '{}'", self.server_name))?;
        
        tracing::info!("MCP >> '{}' : {}", self.server_name, message.chars().take(100).collect::<String>());
        writer.write_all(format!("{}\n", message).as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }
    
    async fn receive(&self) -> Result<String> {
        let line = self.read_line().await?;
        tracing::info!("MCP << '{}' : {}", self.server_name, line.chars().take(100).collect::<String>());
        Ok(line)
    }
    
    async fn close(&self) -> Result<()> {
        let mut process_lock = self.process.lock().await;
        if let Some(mut child) = process_lock.take() {
            child.kill().await.map_err(|e| anyhow!("Failed to kill MCP server '{}': {}", self.server_name, e))?;
            tracing::info!("MCP server '{}' stopped", self.server_name);
        }
        
        *self.writer.lock().await = None;
        *self.reader.lock().await = None;
        Ok(())
    }
}

// ============================================================================
// SSE Transport (HTTP)
// ============================================================================

/// SSE 传输 - 通过 HTTP Server-Sent Events 通信
pub struct SseTransport {
    /// 基础 URL
    base_url: String,
    /// HTTP 客户端
    client: reqwest::Client,
    /// 服务器名称
    server_name: String,
    /// 请求超时
    timeout_ms: u64,
}

impl SseTransport {
    /// 创建 SSE 传输
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        timeout_ms: Option<u64>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            server_name: name.into(),
            timeout_ms: timeout_ms.unwrap_or(30000),
        }
    }
    
    /// 发送 HTTP 请求
    async fn send_http(&self, body: &str) -> Result<String> {
        let url = format!("{}/mcp", self.base_url);
        
        let response = timeout(
            Duration::from_millis(self.timeout_ms),
            self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send()
        ).await
            .map_err(|_| anyhow!("Request timeout for MCP server '{}'", self.server_name))?
            .map_err(|e| anyhow!("HTTP error for MCP server '{}': {}", self.server_name, e))?;
        
        let text = response.text().await?;
        Ok(text)
    }
}

#[async_trait]
impl Transport for SseTransport {
    async fn send(&self, message: &str) -> Result<String> {
        self.send_http(message).await
    }
    
    async fn notify(&self, message: &str) -> Result<()> {
        // SSE 通知也是通过 HTTP POST
        self.send_http(message).await?;
        Ok(())
    }
    
    async fn receive(&self) -> Result<String> {
        // SSE 需要等待 HTTP 响应，通常 send 已包含响应
        // 这里作为简化实现，实际 SSE 场景可能需要单独处理
        Err(anyhow!("SSE receive not implemented - use send() for request/response"))
    }
    
    async fn close(&self) -> Result<()> {
        // HTTP 连接无需关闭
        Ok(())
    }
}

// ============================================================================
// Transport Factory
// ============================================================================

/// 传���配置
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Stdio 传输配置
    Stdio {
        command: String,
        args: Vec<String>,
        env: Option<Vec<(String, String)>>,
    },
    /// SSE 传输配置
    Sse {
        url: String,
        timeout_ms: Option<u64>,
    },
}

impl TransportConfig {
    /// 创建 stdio 配置
    pub fn stdio(command: impl Into<String>, args: Vec<String>) -> Self {
        Self::Stdio {
            command: command.into(),
            args,
            env: None,
        }
    }
    
    /// 创建 SSE 配置
    pub fn sse(url: impl Into<String>) -> Self {
        Self::Sse {
            url: url.into(),
            timeout_ms: None,
        }
    }
}

/// 创建传输实例
pub async fn create_transport(
    server_name: &str,
    config: &TransportConfig,
) -> Result<Box<dyn Transport>> {
    match config {
        TransportConfig::Stdio { command, args, env } => {
            Ok(Box::new(StdioTransport::spawn(
                server_name,
                command,
                args,
                env.clone(),
            ).await?))
        }
        TransportConfig::Sse { url, timeout_ms } => {
            Ok(Box::new(SseTransport::new(
                server_name,
                url,
                *timeout_ms,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transport_config_stdio() {
        let config = TransportConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp".into()]);
        match config {
            TransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Stdio variant"),
        }
    }
    
    #[test]
    fn test_transport_config_sse() {
        let config = TransportConfig::sse("http://localhost:3000");
        match config {
            TransportConfig::Sse { url, .. } => {
                assert_eq!(url, "http://localhost:3000");
            }
            _ => panic!("Expected Sse variant"),
        }
    }
}