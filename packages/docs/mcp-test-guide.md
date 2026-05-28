# MCP 浏览器自动化测试指南

测试 Playwright MCP 打开百度功能。

---

## 📋 测试前准备

### 1. 安装 Node.js

Playwright MCP 需要 Node.js 环境。

**检查 Node.js 是否已安装**：
```bash
node --version
npm --version
```

**如果未安装**：
- Windows: https://nodejs.org/ 下载安装
- macOS: `brew install node`
- Linux: `sudo apt install nodejs npm`

**版本要求**：
- Node.js >= 16.x
- npm >= 8.x

### 2. 编译项目

```bash
# 编译 release 版本
cargo build --release

# 或者编译 TUI 包
cargo build --release --package matrixcode-tui
```

**验证编译成功**：
```bash
# 检查可执行文件
ls -lh target/release/matrixcode-tui.exe

# 或者运行帮助
./target/release/matrixcode-tui.exe --help
```

---

## 🚀 测试步骤

### 方式 1：命令行启动（推荐）

**Step 1: 启动 TUI 并加载 Playwright MCP**
```bash
# Windows
.\target\release\matrixcode-tui.exe --mcp "playwright:npx -y @playwright/mcp@latest"

# Linux/macOS
./target/release/matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"
```

**首次启动会看到**：
```
🔗 正在连接 MCP 'playwright'...
✓ MCP 'playwright' 已连接 (23 工具)
```

**Step 2: 查看 MCP 状态**

在 TUI 界面输入：
```
/mcp
```

应该显示：
```
📋 MCP Servers:
  • playwright ✓ 运行中 (23 工具)
```

**Step 3: 测试打开百度**

在 TUI 界面输入：
```
使用 Playwright 打开百度 https://www.baidu.com
```

**预期结果**：
- Agent 会调用 `browser_navigate` 工具
- 浏览器自动打开百度首页
- Agent 返回操作成功信息

---

### 方式 2：配置文件启动

**Step 1: 创建配置文件**
```bash
# 复制示例配置
cp mcp.example.toml mcp.toml
```

**Step 2: 编辑配置文件**

打开 `mcp.toml`，确保：
```toml
[servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]
enabled = true  # 确保是 true
```

**Step 3: 启动 TUI**
```bash
# Windows
.\target\release\matrixcode-tui.exe

# Linux/macOS
./target/release/matrixcode-tui
```

**Step 4: 测试打开百度**

同方式 1 的 Step 2 和 Step 3。

---

## 🧪 测试用例

### 测试用例 1：打开百度

**输入**：
```
使用 Playwright 打开百度 https://www.baidu.com
```

**预期**：
- 浏览器打开百度首页
- Agent 返回成功信息

---

### 测试用例 2：打开百度并搜索

**输入**：
```
使用 Playwright 打开百度，在搜索框输入 "MatrixCode" 并搜索
```

**预期**：
- Agent 调用 `browser_navigate` 打开百度
- Agent 调用 `browser_click` 点击搜索框
- Agent 调用 `browser_type` 输入 "MatrixCode"
- Agent 调用 `browser_click` 点击搜索按钮
- 浏览器显示搜索结果

---

### 测试用例 3：截图保存

**输入**：
```
使用 Playwright 打开百度并截图
```

**预期**：
- Agent 调用 `browser_navigate` 打开百度
- Agent 调用 `browser_screenshot` 截图
- 返回截图信息

---

### 测试用例 4：查看可用工具

**输入**：
```
你有哪些 Playwright 工具可用？
```

**预期**：
Agent 列出 Playwright MCP 提供的工具：
- browser_navigate
- browser_click
- browser_type
- browser_screenshot
- browser_scroll
- 等等...

---

## ⚠️ 常见问题

### 问题 1：首次启动慢

**现象**：
```
🔗 正在连接 MCP 'playwright'...
[等待很久...]
```

**原因**：
首次运行时，npx 需要下载 `@playwright/mcp` 包。

**解决**：
- 等待下载完成（通常 1-2 分钟）
- 检查网络连接

---

### 问题 2：Node.js 未安装

**现象**：
```
❌ MCP 'playwright' 连接失败: npx: command not found
```

**解决**：
安装 Node.js（见"测试前准备"部分）。

---

### 问题 3：浏览器未打开

**现象**：
Agent 返回成功，但浏览器没有打开。

**原因**：
Playwright MCP 默认使用无头模式（headless）。

**解决**：
```
让浏览器以非无头模式运行，可以看到浏览器窗口
```

或者配置 Playwright MCP 环境变量。

---

### 问题 4：MCP 连接失败

**现象**：
```
❌ MCP 'playwright' 连接失败: timeout
```

**解决**：
1. 检查网络连接
2. 增加超时时间：
   ```toml
   [settings]
   connect_timeout_ms = 60000  # 60 秒
   ```
3. 手动测试 MCP server：
   ```bash
   npx -y @playwright/mcp@latest
   ```

---

### 问题 5：配置文件未加载

**现象**：
启动后状态栏没有显示 `MCP:1`

**解决**：
1. 确认配置文件位置：
   - 项目根目录：`./mcp.toml`
   - 用户目录：`~/.matrixcode/mcp.toml`

2. 确认 `enabled = true`：
   ```toml
   [servers.playwright]
   enabled = true
   ```

---

## 📊 验证检查清单

测试前请确认：

- [ ] Node.js 已安装（`node --version`）
- [ ] npm 已安装（`npm --version`）
- [ ] 项目已编译（`cargo build --release`）
- [ ] API Key 已配置（`~/.matrix/config.json`）
- [ ] 网络连接正常

测试步骤：

- [ ] 启动 TUI（`--mcp` 参数或配置文件）
- [ ] 状态栏显示 `MCP:1`
- [ ] `/mcp` 命令显示 Playwright 状态
- [ ] 输入测试用例命令
- [ ] 浏览器自动打开
- [ ] Agent 返回成功信息

---

## 🎯 测试成功标准

测试通过的标准：

1. ✅ TUI 启动成功，状态栏显示 `MCP:1`
2. ✅ `/mcp` 命令显示 Playwright 运行中
3. ✅ Agent 能够调用 Playwright 工具
4. ✅ 浏览器自动打开百度
5. ✅ 搜索功能正常工作
6. ✅ 截图功能正常工作

---

## 📝 测试日志示例

```
# 启动
$ ./target/release/matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest"

# TUI 界面显示
 claude-sonnet-4 │ Ask │ ███ 45% 120k/200k │ out 15k │ MCP:1 │ Ready

# 用户输入
User: /mcp

# 系统响应
📋 MCP Servers:
  • playwright ✓ 运行中 (23 工具)

# 用户输入
User: 使用 Playwright 打开百度 https://www.baidu.com

# Agent 响应
💭 正在调用 browser_navigate...

✅ 已成功打开百度首页

工具调用:
  browser_navigate(https://www.baidu.com)
```

---

## 🔧 调试技巧

### 1. 查看 MCP 日志

MCP server 的日志会输出到 stderr：
```bash
# 启动时重定向日志
./target/release/matrixcode-tui --mcp "playwright:npx -y @playwright/mcp@latest" 2>mcp.log
```

### 2. 手动测试 MCP server

```bash
# 直接运行 MCP server
npx -y @playwright/mcp@latest

# 应该看到类似输出
{"jsonrpc":"2.0","method":"tools/list",...}
```

### 3. 检查工具列表

在 TUI 中输入：
```
列出所有 Playwright 工具
```

Agent 会返回可用工具列表。

---

## 🎉 测试完成

如果所有测试用例通过，恭喜！MCP 浏览器自动化功能正常工作。

**下一步**：
- 尝试其他 MCP servers（filesystem、memory 等）
- 探索更多 Playwright 工具
- 查看完整文档：`docs/mcp-guide.md`

---

**测试愉快！** 🚀