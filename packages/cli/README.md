# MatrixCode CLI

智能代码代理 CLI 工具，支持多模型配置、智能上下文压缩、跨会话记忆和工作流引擎。

## 安装

### 通过 npm 安装（推荐）

```bash
npm install -g @bigfishnpm/matrixcode
```

npm 包会自动下载对应平台的预编译二进制文件。

### 通过 Cargo 安装

```bash
cargo install matrixcode
```

### 从源码构建

```bash
# 在项目根目录
cargo build --release

# 或使用 Task
task build
```

## 使用方式

### 终端交互模式 (TUI)

```bash
# 启动交互式终端
matrixcode

# 或显式指定模式
matrixcode --mode terminal
matrixcode --mode tui
```

TUI 模式特性：
- 流式响应渲染
- Markdown 格式化输出
- 代码语法高亮
- 工具调用显示
- 思考过程展示

### 单次问答

```bash
# 直接提问
matrixcode "分析这个项目的结构"

# JSON 输出模式（适合脚本集成）
matrixcode --mode service "解释这个函数"
matrixcode --mode json "生成单元测试"
```

### Daemon 服务模式

用于 VS Code 扩展或其他客户端集成：

```bash
# 启动 Daemon
matrixcode --mode daemon

# 通过 stdin 发送 JSON 请求
echo '{"type":"chat","content":"test"}' | matrixcode --mode daemon
```

支持的请求类型：
- `chat` - 聊天请求
- `resume` - 恢复会话
- `new_session` - 新建会话
- `cancel` - 取消当前请求

### 会话管理

```bash
# 查看历史会话列表
matrixcode --list-sessions

# 交互式选择恢复会话
matrixcode --resume

# 从指定会话恢复
matrixcode --session <session-id>
```

## 配置

### 配置文件

创建 `~/.matrix/config.json`：

```json
{
  "provider": "anthropic",
  "apiKey": "your-api-key",
  "model": "claude-sonnet-4-20250514",
  "compressModel": "claude-3-5-haiku-20241022",
  "maxTokens": 16384,
  "think": true,
  "showThinking": true,
  "showToolUse": true,
  "approveMode": "auto",
  "memoryEnabled": true
}
```

### 环境变量

在 `.env` 文件或环境变量中配置：

```env
PROVIDER=anthropic
API_KEY=sk-ant-your-key
MODEL=claude-sonnet-4-20250514
COMPRESS_MODEL=claude-3-5-haiku-20241022
MAX_TOKENS=16384
THINK=true
```

### 配置项说明

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| `provider` | LLM 提供者 (anthropic/openai) | anthropic |
| `apiKey` | API 密钥 | - |
| `model` | 主模型 | claude-sonnet-4-20250514 |
| `compressModel` | 压缩模型（可选） | - |
| `maxTokens` | 最大输出 tokens | 4096 |
| `think` | 启用扩展思考模式 | true |
| `approveMode` | 工具审批模式 (auto/yetask/auto-edits) | auto |
| `memoryEnabled` | 启用记忆系统 | true |

## CLI 参数

### 全局参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-m, --mode` | 运行模式 | `terminal` |
| `-r, --resume` | 交互式恢复会话 | - |
| `--resume-id <ID>` | 恢复指定会话 | - |
| `--list-sessions` | 列出历史会话 | - |
| `-c, --continue-session` | 继续上次会话 | - |
| `--skills-dir <PATH>` | 额外技能目录 | - |
| `--think [true/false]` | 扩展思考模式 | 配置值 |
| `--max-tokens` | 最大输出 tokens | 16384 |

### 运行模式

| 模式 | 说明 |
|------|------|
| `terminal` / `tui` | 终端交互模式（TUI 界面） |
| `service` / `json` | JSON 输出模式（脚本集成） |
| `daemon` | Daemon 服务模式（VS Code 扩展） |

### 子命令

```bash
# 聊天
matrixcode chat --message "你的问题"

# 快速操作
matrixcode quick-action --action explain --file src/main.rs

# 新建会话
matrixcode new-session

# 查看历史
matrixcode history

# 查看状态
matrixcode status

# Workflow 命令
matrixcode workflow discover          # 发现可用 workflow
matrixcode workflow discover "审查"   # 搜索匹配
matrixcode workflow run file.yaml     # 运行 workflow
matrixcode workflow run file.yaml --inputs '{"key": "value"}'
matrixcode workflow list              # 列出历史
matrixcode workflow status <id>       # 查看状态
matrixcode workflow resume <id>       # 恢复暂停的 workflow
matrixcode workflow abort <id>        # 终止运行中的 workflow
matrixcode workflow export file.yaml --format mermaid
```

## 开发

### 测试

```bash
# 运行所有测试
cargo test --all

# 仅测试 core 模块
cargo test -p matrixcode-core

# 仅测试 tui 模块
cargo test -p matrixcode-tui
```

### 构建

```bash
# Debug 构建
cargo build

# Release 构建
cargo build --release

# 安装到本地
cargo install --path .
```

### 发布

```bash
# 发布到 crates.io
cargo publish -p matrixcode-core
cargo publish -p matrixcode-tui
cargo publish -p matrixcode

# 发布到 npm
cd packages/npm && npm publish --access public
```

## 项目结构

```
packages/cli/
├── Cargo.toml           # CLI 包配置
├── src/
│   ├── main.rs          # CLI 入口
│   ├── terminal_mode/   # 终端交互模式
│   ├── commands/        # 命令处理
│   │   ├── daemon.rs    # Daemon 模式
│   │   ├── service.rs   # JSON 服务模式
│   │   └── workflow.rs  # Workflow 命令
│   ├── display.rs       # 输出渲染
│   └── helpers.rs       # 辅助函数
└
├── packages/npm/        # npm 发布包
│   ├── package.json
│   └── scripts/         # 二进制下载脚本
```

## 文档

- [核心功能](../docs/)
- [工作流设计](../docs/openmatrix/2026-05-25-workflow-design.md)
- [压缩优化](../docs/openmatrix/2025-05-23-compression-optimization-design.md)
- [记忆架构](../docs/SESSION_MEMORY_ARCHITECTURE.md)

## 许可证

MIT License