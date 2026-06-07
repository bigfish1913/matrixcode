# MatrixCode MCP 工具集成 Demo

Model Context Protocol (MCP) 让 Agent 能够通过外部工具无限扩展能力。

## 🎯 Demo 目标

使用 Playwright MCP 实现浏览器自动化：
1. 打开网页
2. 自动填表
3. 截图保存

## 📋 准备工作

### 1. 安装 MCP 工具

```bash
# 确保 Node.js 已安装
npm install -g npx

# 测试 Playwright MCP 是否可用
npx -y @playwright/mcp@latest --help
```

### 2. 创建 MCP 配置

```bash
# 复制配置模板
cp mcp.example.toml mcp.toml
```

配置内容（`mcp.toml`）：
```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true
```

### 3. 启动 MatrixCode

**方式一：配置文件启动**
```bash
matrixcode-tui
# 自动读取 mcp.toml 配置
```

**方式二：命令行启动**
```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

## 🚀 完整工作流程

### 启动 MatrixCode CLI

```bash
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

### 实际对话过程

**用户输入：**
```
打开浏览器，访问 https://example.com，截图保存为 screenshot.png
```

**MatrixCode 响应：**
```
💭 Thinking: 分析任务...
   → 需要使用 Playwright MCP
   → 执行浏览器导航和截图

正在启动 Playwright MCP 工具...

🔧 MCP Tool: browser_navigate
参数: { url: "https://example.com" }
执行中...

✅ 网页已打开: https://example.com

🔧 MCP Tool: browser_screenshot
参数: { path: "screenshot.png" }
执行中...

✅ 截图已保存: screenshot.png
```

### 更多 MCP 工具调用

**用户输入：**
```
在网页中查找所有链接，点击第一个链接
```

**MatrixCode 响应：**
```
🔧 MCP Tool: browser_query_selector_all
参数: { selector: "a" }
返回: 找到 5 个链接

🔧 MCP Tool: browser_click
参数: { selector: "a:nth-child(1)" }
执行中...

✅ 已点击第一个链接
当前 URL: https://www.iana.org/domains/reserved
```

**用户输入：**
```
在页面中输入文本 "MatrixCode Test"
```

**MatrixCode 响应：**
```
🔧 MCP Tool: browser_query_selector
参数: { selector: "input[type='text']" }
返回: 找到输入框

🔧 MCP Tool: browser_type
参数: { 
  selector: "input[type='text']",
  text: "MatrixCode Test"
}
执行中...

✅ 文本已输入
```

## 📊 Playwright MCP 工具列表

| 工具名 | 功能 | 示例用法 |
|--------|------|---------|
| `browser_navigate` | 打开网页 | `navigate("https://example.com")` |
| `browser_click` | 点击元素 | `click("button.submit")` |
| `browser_type` | 输入文本 | `type("input#name", "MatrixCode")` |
| `browser_screenshot` | 截图 | `screenshot("screenshot.png")` |
| `browser_query_selector` | 查找元素 | `query_selector("h1")` |
| `browser_query_selector_all` | 查找所有元素 | `query_selector_all("a")` |
| `browser_scroll` | 滚动页面 | `scroll(0, 500)` |
| `browser_wait` | 等待元素 | `wait("div.content", 5000)` |

完整工具列表（23 个工具）：
- browser_navigate
- browser_click
- browser_type
- browser_screenshot
- browser_query_selector
- browser_query_selector_all
- browser_scroll
- browser_wait
- browser_fill
- browser_select
- browser_hover
- browser_press
- browser_drag
- browser_drop
- browser_check
- browser_uncheck
- browser_set_input_files
- browser_focus
- browser_blur
- browser_get_text
- browser_get_attribute
- browser_get_inner_html
- browser_evaluate

## ✨ MCP 集成的优势

1. **无缝集成**: Agent 自动识别并调用 MCP 工具
2. **类型安全**: MCP 工具参数有严格的类型定义
3. **实时反馈**: 工具执行结果实时返回给 Agent
4. **多工具组合**: 一个任务可以使用多个 MCP 工具

## 🎯 实际应用场景

| 场景 | MCP 工具 | 用途 |
|------|---------|------|
| 网站测试 | Playwright | 自动化 UI 测试 |
| 数据采集 | Puppeteer | 爬取网页数据 |
| 文件管理 | filesystem | 批量文件操作 |
| 数据库操作 | postgres/sqlite | 数据查询和操作 |
| GitHub 集成 | github | 自动 PR/Issue 管理 |
| 记忆存储 | memory | 跨会话数据存储 |

## 🔧 配置多个 MCP Server

```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true

[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/project"]
enabled = true

[servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
enabled = true
```

## ⚠️ 安全注意事项

1. **文件系统访问**: filesystem MCP 需明确指定允许访问的目录
2. **API Token**: github/gitlab MCP 需设置环境变量 Token
3. **数据库连接**: postgres/sqlite MCP 需验证连接字符串

## 🔗 相关文档

- [MCP 配置详解](../docs.html#mcp)
- [更多 MCP Server](https://github.com/modelcontextprotocol)
- [Playwright 文档](https://playwright.dev)

## 📊 测试验证

运行以下命令验证 MCP 配置：

```bash
# 检查 MCP 配置文件
cat mcp.toml

# 测试 MCP 工具可用性
npx -y @playwright/mcp@latest --help

# 启动 MatrixCode 并测试
matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

预期输出：
```
✅ MCP server initialized
✅ 23 tools available from playwright
✅ MatrixCode TUI started
```