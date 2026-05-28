# MCP 快速入门指南

5 分钟快速上手 MatrixCode MCP 功能。

---

## 🎯 什么是 MCP？

MCP (Model Context Protocol) 是一种协议，让 AI Agent 能够通过外部工具扩展能力，例如：
- 🌐 浏览器自动化（打开网页、点击、输入、截图）
- 📁 文件系统访问（读写文件、搜索目录）
- 💾 记忆存储（保存和检索信息）
- 🔗 API 集成（GitHub、GitLab、Slack 等）

---

## 🚀 快速开始（2 种方式）

### 方式 1：命令行启动（推荐）

**一键启动 Playwright MCP**：
```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

**多个 MCP servers**：
```bash
matrixcode-tui \
  --mcp "playwright:npx -y @playwright/mcp@latest" \
  --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /home/user/project"
```

### 方式 2：配置文件启动

**Step 1: 复制配置文件**
```bash
cp mcp.example.toml mcp.toml
```

**Step 2: 编辑配置文件**
```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true  # 改为 true
```

**Step 3: 启动 TUI**
```bash
matrixcode-tui
```

---

## ✅ 验证 MCP 已连接

启动 TUI 后，查看状态栏：

```
 claude-sonnet-4 │ Ask │ ███ 45% 120k/200k │ out 15k │ MCP:1 │ Ready
                                                        ^^^^^
                                                    MCP 已连接
```

输入 `/mcp` 查看详细信息：
```
User: /mcp

System: 📋 MCP Servers:
  • playwright ✓ 运行中 (23 工具)
```

---

## 🎮 浏览器自动化示例

**任务：打开百度并搜索**

**Step 1: 启动 TUI**
```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

**Step 2: 对话**
```
User: 使用 Playwright 打开百度并搜索 "MatrixCode"

Agent: 
✅ 打开百度
[调用 browser_navigate 打开 https://www.baidu.com]

✅ 点击搜索框
[调用 browser_click]

✅ 输入搜索内容
[调用 browser_type 输入 "MatrixCode"]

✅ 点击搜索按钮
[调用 browser_click]
```

---

## 📁 文件系统访问示例

**任务：列出项目文件**

**Step 1: 启动 TUI**
```bash
matrixcode-tui --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /home/user/project"
```

**Step 2: 对话**
```
User: 列出项目根目录的所有文件

Agent: [调用 filesystem list_directory 工具]

📁 /home/user/project/
  ├── src/
  ├── tests/
  ├── Cargo.toml
  └── README.md
```

---

## 🎯 常用 MCP Servers

| MCP Server | 功能 | 工具数 | 启动命令 |
|------------|------|--------|----------|
| **playwright** | 浏览器自动化 | 23 | `--mcp "playwright:npx -y @playwright/mcp@latest"` |
| **filesystem** | 文件系统访问 | 多个 | `--mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem /path"` |
| **memory** | 键值存储 | 多个 | `--mcp "memory:npx -y @modelcontextprotocol/server-memory"` |
| **github** | GitHub API | 多个 | `--mcp "github:npx -y @modelcontextprotocol/server-github"` |
| **postgres** | PostgreSQL | 多个 | `--mcp "postgres:npx -y @modelcontextprotocol/server-postgres <url>"` |

---

## 💡 使用技巧

### 1. 查看可用工具

```
User: 你有哪些 MCP 工具可用？

Agent: 我有以下 Playwright 工具：
- browser_navigate: 打开网页
- browser_click: 点击元素
- browser_type: 输入文本
- browser_screenshot: 截图
...
```

### 2. 运行时管理

```
User: 添加一个新的 memory MCP server

Agent: [调用 add_mcp_server() API]
System: 🔗 MCP 'memory' 已连接

User: 移除 filesystem MCP server

Agent: [调用 remove_mcp_server() API]
System: 🔌 MCP 'filesystem' 已移除
```

### 3. 多 MCP 协同

```
User: 打开百度，截图并保存到项目目录

Agent: 
[调用 playwright browser_navigate]
[调用 playwright browser_screenshot]
[调用 filesystem write_file]
```

---

## ⚠️ 注意事项

1. **Node.js 环境**：大多数 MCP servers 需要 Node.js 和 npx
2. **首次启动慢**：第一次运行时会下载 MCP server 包
3. **安全性**：filesystem MCP 只能访问指定目录
4. **API Keys**：GitHub、Slack 等 MCP 需要配置 API tokens

---

## 📖 更多资源

- **详细文档**：[docs/mcp-guide.md](docs/mcp-guide.md)
- **配置示例**：[mcp.example.toml](mcp.example.toml)
- **MCP 官方**：https://modelcontextprotocol.io/
- **Servers 列表**：https://github.com/modelcontextprotocol/servers

---

**开始你的 MCP 之旅！** 🚀

```bash
# 一键启动
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"

# 然后对话
User: 打开百度
```