# MCP (Model Context Protocol) Integration

## Overview

MCP 是 Anthropic 提出的标准协议，用于连接外部工具和数据源。本模块实现了完整的 MCP 客户端，可以连接任意 MCP 服务器并将其工具透明地映射为 MatrixCode 内置工具。

## Architecture

```
MatrixCode Agent
    │
    ├── Tool System
    │   ├── Built-in Tools (read, write, edit, grep, etc.)
    │   ├── CodeGraph Tools (code_search, code_callers, etc.)
    │   └── MCP Tool Wrapper (transparent proxy)
    │       │
    │       └── McpClient
    │           │
    │           ├── Stdio Transport (stdin/stdout)
    │           └── SSE Transport (HTTP)
    │
    └── External MCP Servers
        ├── Playwright MCP (@playwright/mcp)
        ├── Filesystem MCP (@modelcontextprotocol/server-filesystem)
        ├── GitHub MCP (@modelcontextprotocol/server-github)
        └── Any other MCP server...
```

## Quick Start

### 1. Create Configuration

Create `mcp.toml` in your project root:

```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true
```

### 2. Connect to MCP Server

```rust
use matrixcode_core::mcp::{McpConfig, McpToolManager};

async fn connect_mcp() -> anyhow::Result<()> {
    // Load config
    let config = McpConfig::from_file("mcp.toml")?;
    
    // Create manager
    let manager = McpToolManager::new();
    
    // Connect servers
    for (key, server_config) in config.enabled_servers() {
        let transport = server_config.to_transport_config()?;
        let tools = manager.connect_server(&server_config.get_name(&key), transport).await?;
        println!("Connected: {} tools from {}", tools.len(), key);
    }
    
    // Use tools (integrate with Agent)
    // ...
    
    // Shutdown
    manager.shutdown().await;
    Ok(())
}
```

### 3. Convenience Functions

```rust
// Connect Playwright directly
let tools = matrixcode_core::mcp::connect_playwright().await?;

// Connect all from auto-discovered config
let tools = matrixcode_core::mcp::connect_all_from_config(std::path::Path::new(".")).await?;
```

## Available MCP Servers

### Playwright MCP
Browser automation:
- `browser_navigate` - Navigate to URL
- `browser_click` - Click element
- `browser_screenshot` - Take screenshot
- `browser_fill` - Fill form field
- `browser_wait` - Wait for element

```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
```

### Filesystem MCP
File operations (sandboxed):
- `read_file` - Read file content
- `write_file` - Write file content
- `list_directory` - List directory contents
- `search_files` - Search files by pattern

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/sandbox"]
```

### GitHub MCP
GitHub operations:
- `search_repositories` - Search repos
- `get_issue` - Get issue details
- `create_pull_request` - Create PR
- `list_commits` - List commits

```toml
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "your-token" }
```

## Transport Types

### Stdio (Recommended)
Communication via subprocess stdin/stdout:

```toml
[servers.example]
command = "npx"
args = ["-y", "some-mcp-server"]
```

### SSE (HTTP)
Communication via HTTP Server-Sent Events:

```toml
[servers.remote]
url = "http://localhost:3000/mcp"
timeout_ms = 60000
```

## Module Structure

| File | Purpose |
|------|---------|
| `types.rs` | MCP protocol types (JSON-RPC, Tool, etc.) |
| `transport.rs` | Transport layer (Stdio, SSE) |
| `client.rs` | MCP client (connection, handshake, tool calls) |
| `proxy.rs` | Tool wrapper (MCP → MatrixCode Tool) |
| `config.rs` | Configuration management |
| `mod.rs` | Module entry point |

## Risk Levels

MCP tools are assigned risk levels based on operation type:

- **Safe**: `read`, `list`, `get` operations
- **Mutating**: `browser`, `navigate`, `click` operations
- **Dangerous**: `write`, `delete`, `create` operations

## Error Handling

MCP client handles errors gracefully:

1. Connection errors → Server marked as failed
2. Tool errors → Returned as `is_error` in response
3. Timeout → Configurable per server

## Testing

To test MCP integration without external server:

```bash
# Install a simple MCP server
npm install -g @modelcontextprotocol/server-everything

# Create config
[servers.test]
command = "mcp-server-everything"

# Run integration
cargo test --package matrixcode-core --lib mcp
```

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [Playwright MCP](https://github.com/nickyout/playwright-mcp)
- [Official MCP Servers](https://github.com/modelcontextprotocol/servers)