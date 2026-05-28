# MCP (Model Context Protocol) 使用指南

MatrixCode 支持 MCP (Model Context Protocol)，可以通过 MCP servers 扩展 Agent 的能力，例如浏览器自动化、文件系统访问、记忆存储等。

---

## 🚀 快速开始

### 方式 1：命令行参数启动

```bash
# 启动 TUI 并加载 Playwright MCP
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"

# 加载多个 MCP servers
matrixcode-tui \
  --mcp "playwright:npx -y @playwright/mcp@latest" \
  --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /path/to/dir"
```

### 方式 2：配置文件启动

```bash
# 复制示例配置文件到项目根目录
cp mcp.example.toml mcp.toml

# 启动 TUI（自动加载配置）
matrixcode-tui
```

---

## 📋 CLI 参数格式

MatrixCode 支持两种 MCP 参数格式：

### 格式 1：自定义名称

```bash
--mcp "name:command args"
```

示例：
```bash
--mcp "playwright:npx -y @playwright/mcp@latest"
--mcp "my-server:node server.js --port 3000"
```

### 格式 2：自动推断名称

```bash
--mcp "command args"
```

名称会自动取命令的第一部分：
```bash
--mcp "npx -y @playwright/mcp@latest"  # 自动命名为 "npx"
```

---

## 🗂️ 配置文件格式

在项目根目录创建 `mcp.toml` 文件：

```toml
[servers.playwright]
# Playwright 浏览器自动化（23 个工具）
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true

[servers.filesystem]
# 文件系统访问
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/dir"]
enabled = true

[servers.memory]
# 简单的键值记忆存储
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
enabled = false

[settings]
# 自动发现 MCP servers（默认：true）
auto_discover = true

# 连接超时（毫秒，默认：30000）
connect_timeout_ms = 30000
```

---

## 🔧 常用 MCP Servers

### 1. Playwright（浏览器自动化）

**工具数量**：23 个工具

**主要功能**：
- `browser_navigate` - 打开网页
- `browser_click` - 点击元素
- `browser_type` - 输入文本
- `browser_screenshot` - 截图
- `browser_scroll` - 滚动页面
- 等等...

**示例用法**：
```
User: 使用 Playwright 打开百度
Agent: [调用 browser_navigate 打开 https://www.baidu.com]

User: 点击搜索框并输入 "MatrixCode"
Agent: [调用 browser_click + browser_type]
```

**配置**：
```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true
```

**启动命令**：
```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

---

### 2. Filesystem（文件系统访问）

**工具数量**：多个文件操作工具

**主要功能**：
- 读写文件
- 创建/删除目录
- 搜索文件
- 等等...

**配置**：
```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
enabled = true
```

**启动命令**：
```bash
matrixcode-tui --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /path/to/dir"
```

---

### 3. Memory（记忆存储）

**工具数量**：键值存储工具

**主要功能**：
- 存储和检索记忆
- 简单的键值数据库

**配置**：
```toml
[servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
enabled = true
```

---

## 💻 TUI MCP 功能

### 状态栏显示

状态栏会显示已连接的 MCP servers 数量：

```
 claude-sonnet-4 │ Ask │ ███ 45% 120k/200k │ out 15k │ MCP:1 │ Ready
```

- `MCP:1` 表示已连接 1 个 MCP server
- `MCP:2` 表示已连接 2 个 MCP servers

### 查看详细状态

输入 `/mcp` 命令查看详细信息：

```
User: /mcp

System: 📋 MCP Servers:
  • playwright ✓ 运行中 (23 工具)
  • filesystem ✓ 运行中 (15 工具)
```

### 查看帮助

输入 `/help` 查看 MCP 命令：

```
/mcp      - List MCP servers status
```

---

## 🎯 使用示例

### 示例 1：打开百度并搜索

**启动**：
```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

**对话**：
```
User: 使用 Playwright 打开百度并搜索 "MatrixCode"

Agent: 
[调用 browser_navigate 打开 https://www.baidu.com]
[调用 browser_click 点击搜索框]
[调用 browser_type 输入 "MatrixCode"]
[调用 browser_click 点击搜索按钮]
```

---

### 示例 2：管理项目文件

**启动**：
```bash
matrixcode-tui --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /home/user/project"
```

**对话**：
```
User: 列出项目根目录的所有文件

Agent: [调用 filesystem list_directory 工具]
```

---

### 示例 3：多 MCP 协同

**启动**：
```bash
matrixcode-tui \
  --mcp "playwright:npx -y @playwright/mcp@latest" \
  --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /home/user/project"
```

**对话**：
```
User: 打开百度，截图并保存到项目目录

Agent: 
[调用 playwright browser_navigate 打开百度]
[调用 playwright browser_screenshot 截图]
[调用 filesystem write_file 保存截图]
```

---

## 🔍 运行时 API

MatrixCode Agent 拥有运行时 MCP 管理能力：

### 添加 MCP Server

```
User: 添加一个新的 MCP server "memory"

Agent: [调用 add_mcp_server() API]
System: 🔗 MCP 'memory' 已连接
```

### 移除 MCP Server

```
User: 移除 "filesystem" MCP server

Agent: [调用 remove_mcp_server() API]
System: 🔌 MCP 'filesystem' 已移除
```

### 查看状态

```
User: 列出当前所有的 MCP servers

Agent: [调用 mcp_server_status() API]
System: 📋 MCP Servers:
  • playwright ✓ 运行中 (23 工具)
  • memory ✓ 运行中 (5 工具)
```

---

## ⚠️ 注意事项

1. **Node.js 环境**：大多数 MCP servers 需要 Node.js 和 npx
2. **网络连接**：首次启动时需要下载 MCP server 包
3. **超时设置**：默认超时 30 秒，可在配置文件中调整
4. **安全性**：filesystem MCP 只能访问指定的目录

---

## 📖 更多 MCP Servers

官方 MCP servers 列表：
- [Playwright](https://github.com/modelcontextprotocol/servers/tree/main/src/playwright)
- [Filesystem](https://github.com/modelcontextprotocol/servers/tree/main/src filesystem)
- [Memory](https://github.com/modelcontextprotocol/servers/tree/main/src/memory)
- [GitHub](https://github.com/modelcontextprotocol/servers/tree/main/src/github)
- [GitLab](https://github.com/modelcontextprotocol/servers/tree/main/src/gitlab)
- [PostgreSQL](https://github.com/modelcontextprotocol/servers/tree/main/src/postgres)
- [Slack](https://github.com/modelcontextprotocol/servers/tree/main/src/slack)
- [Google Drive](https://github.com/modelcontextprotocol/servers/tree/main/src/gdrive)

---

## 🔗 相关链接

- [MCP 官方文档](https://modelcontextprotocol.io/)
- [MCP Servers 列表](https://github.com/modelcontextprotocol/servers)
- [MatrixCode GitHub](https://github.com/your-org/matrixcode)

---

**MCP 功能让 MatrixCode Agent 具备无限扩展能力！** 🚀