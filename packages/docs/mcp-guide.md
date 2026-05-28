# MCP (Model Context Protocol) 配置指南

## 概述

MatrixCode 支持通过 MCP 协议集成外部工具，允许使用 Playwright 等第三方工具扩展功能。

## 配置文件位置

MCP 配置支持两个层级，自动合并：

| 位置 | 优先级 | 用途 |
|------|--------|------|
| **项目级** `./mcp.toml` | 高 | 项目特定工具配置 |
| **用户级** `~/.matrixcode/mcp.toml` | 低 | 全局共享配置 |

项目配置会覆盖用户配置中的同名服务器。

## 配置文件格式

### TOML 格式 (推荐)

```toml
# MCP 服务器配置
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true

[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
enabled = false

# 全局设置
[settings]
auto_discover = true          # 自动发现配置文件 (默认: true)
connect_timeout_ms = 30000    # 连接超时 (默认: 10000)
```

### JSON 格式 (兼容 VS Code)

```json
{
  "servers": {
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"],
      "enabled": true
    }
  },
  "settings": {
    "auto_discover": true,
    "connect_timeout_ms": 30000
  }
}
```

## 服务器配置字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `command` | string | 是* | Stdio 模式的启动命令 |
| `args` | array | 否 | 命令参数 |
| `env` | object | 否 | 环境变量 `{"KEY": "value"}` |
| `url` | string | 是* | SSE 模式的 HTTP URL |
| `timeout_ms` | number | 否 | 请求超时（毫秒），默认 30000 |
| `enabled` | boolean | 否 | 是否启用，默认 true |

\* `command` 和 `url` 二选一

## 常用 MCP 服务器

### Playwright (浏览器自动化)

```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
```

**提供 23 个工具**：
- 导航: `navigate`, `navigate_back`, `tabs`
- 交互: `click`, `hover`, `drag`, `type`, `press_key`
- 表单: `fill_form`, `select_option`, `file_upload`
- 信息: `screenshot`, `snapshot`, `console_messages`
- 执行: `evaluate`, `run_code_unsafe`

### Filesystem (文件系统访问)

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/allowed/path"]
```

### Memory (简单记忆存储)

```toml
[servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
```

## 使用方式

### 方式一：代码调用

```rust
use matrixcode_core::mcp::{McpToolRegistry, load_mcp_config};
use std::path::Path;

// 加载配置
let config = load_mcp_config(Path::new("."));

// 创建懒加载注册表
let registry = McpToolRegistry::from_config(&config);

// 按需启动服务器
if let Some(placeholder) = registry.get_server("playwright") {
    let tools = placeholder.start().await?;
    
    // 使用工具
    for tool in &tools {
        println!("Tool: {}", tool.definition().name);
    }
}

// 关闭
registry.shutdown_all().await;
```

### 方式二：快速连接

```rust
use matrixcode_core::mcp::connect_playwright;

// 直接连接 Playwright
let tools = connect_playwright().await?;
println!("Playwright tools: {}", tools.len());
```

## 懒加载机制

MCP 服务器采用懒加载策略：

1. **注册阶段**：解析配置，创建占位符（不启动进程）
2. **启动阶段**：首次调用时启动服务器
3. **执行阶段**：使用已启动的服务器执行工具
4. **关闭阶段**：会话结束时关闭所有服务器

**优点**：
- 初始化无延迟
- 按需使用资源
- 不使用的服务器不会启动

## Windows 特殊处理

在 Windows 上，`npx`/`npm`/`node` 命令会自动通过 `cmd.exe` 执行：

```
实际命令: cmd.exe /c npx -y @playwright/mcp@latest
```

无需手动配置。

## 故障排查

### 连接超时

如果首次启动超时（Playwright 需下载依赖）：

```toml
[settings]
connect_timeout_ms = 60000  # 增加到 60 秒
```

### 进程未关闭

确保调用 `shutdown_all()` 或让程序正常退出（会自动清理）。

### 工具未发现

检查：
1. 配置文件位置是否正确
2. `enabled = true` 是否设置
3. 命令是否可执行（手动测试：`npx -y @playwright/mcp@latest`）

## 下一步

当前 MCP 工具需要手动调用。后续版本将提供：

1. 自动集成到工具系统
2. 工具提示中显示 MCP 工具
3. Agent 自动选择 MCP 工具

---

**示例配置文件**: `mcp.example.toml`