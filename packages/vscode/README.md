# MatrixCode VS Code Extension

**可定制工作流的 AI 代码助手** - VS Code 侧边栏集成，支持快捷操作和工作流调用。

## 功能

### 💬 侧边栏聊天界面
- 流式响应渲染
- Markdown 格式化输出
- 代码语法高亮
- 思考过程和工具调用可视化

### ⚡ 快速操作
选中代码后一键执行：
- **解释代码** - 分析代码逻辑和用途
- **修复代码** - 识别并修复问题
- **生成测试** - 自动生成单元测试
- **重构代码** - 优化代码结构
- **改进代码** - 提升代码质量

### 📋 工作流集成
在聊天中调用 YAML 定义的工作流：
```
/workflow discover
/workflow run code-review-workflow
```

### 📍 自动上下文
- 自动附加当前文件内容
- 自动附加选中代码
- 智能上下文压缩

## 安装

### VS Code Marketplace

1. 打开 VS Code 扩展市场
2. 搜索 "MatrixCode"
3. 点击安装

### 前置要求

需要安装 MatrixCode CLI：

```bash
# npm
npm install -g @bigfishnpm/matrixcode

# 或 Cargo
cargo install matrixcode
```

## 配置

### VS Code 设置

在 VS Code 设置中配置（`Ctrl+,` 搜索 "MatrixCode"）：

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| `matrixcode.cliPath` | CLI 路径 | `matrixcode` |
| `matrixcode.provider` | LLM 提供者 | `anthropic` |
| `matrixcode.model` | 模型名称 | 空（使用默认） |
| `matrixcode.autoContext` | 自动附加上下文 | `true` |
| `matrixcode.think` | 扩展思考模式 | `true` |
| `matrixcode.maxTokens` | 最大输出 tokens | `16384` |
| `matrixcode.daemonMode` | Daemon 模式 | `true` |
| `matrixcode.showThinking` | 显示思考过程 | `true` |
| `matrixcode.showToolUse` | 显示工具调用 | `true` |

### API Key 配置

创建 `~/.matrix/config.json`：

```json
{
  "provider": "anthropic",
  "apiKey": "your-api-key-here",
  "model": "claude-sonnet-4-20250514"
}
```

或使用环境变量：

```bash
export ANTHROPIC_API_KEY=your-key
```

## 快捷键

| 命令 | Windows | macOS | 说明 |
|------|---------|-------|------|
| Open Chat | `Ctrl+K` | `Cmd+K` | 打开聊天面板 |
| Quick Action | `Ctrl+Shift+K` | `Cmd+Shift+K` | 快速操作（需选中代码） |
| Explain | `Ctrl+Shift+E` | `Cmd+Shift+E` | 解释选中代码 |
| Fix | `Ctrl+Shift+F` | `Cmd+Shift+F` | 修复选中代码 |
| Generate Tests | `Ctrl+Shift+T` | `Cmd+Shift+T` | 生成测试 |
| Refactor | `Ctrl+Shift+R` | `Cmd+Shift+R` | 重构选中代码 |

## 右键菜单

选中代码后右键，选择 MatrixCode 子菜单：
- Explain Code
- Fix Code
- Refactor Selection
- Generate Tests

## 开发与调试

### 快速开始

```bash
# 安装依赖
npm install

# 开发编译（带 sourcemap）
npm run compile-dev

# 监听模式
npm run watch
```

### 调试

1. 在 VS Code 中打开此目录
2. 按 `F5` 启动调试
3. 会打开扩展开发宿主窗口
4. 侧边栏可以看到 MatrixCode 图标

### 调试配置

| 配置名称 | 说明 |
|---------|------|
| Run Extension (Debug) | 单次编译后调试 |
| Run Extension (Watch) | 监听模式调试 |
| Extension Tests | 运行测试 |

### 常用命令

```bash
# 开发编译
npm run compile-dev

# 监听模式
npm run watch

# 生产编译
npm run compile

# 代码检查
npm run lint

# 打包 VSIX
npm run package

# 发布 Marketplace
npm run publish
```

## 项目结构

```
packages/vscode/
├── src/
│   ├── extension.ts      # 扩展入口
│   ├── chatView.ts       # 聊天视图
│   ├── configManager.ts  # 配置管理
│   └── matrixcodeClient.ts # CLI 客户端
├── dist/                 # 编译输出
├── resources/            # 资源文件
└── .vscode/              # VS Code 配置
    ├── launch.json       # 调试配置
    ├── tasks.json        # 任务配置
```

## 特性与 Claude Code 对比

| 功能 | Claude Code Extension | MatrixCode Extension |
|------|----------------------|---------------------|
| 工作流定制 | ❌ 无 | ✅ YAML 定义工作流 |
| 跨会话记忆 | ⚠️ 有限 | ✅ 完整记忆系统 |
| 多模型支持 | ❌ 仅 Claude | ✅ Claude + GPT |
| 成本优化 | ❌ 单一模型 | ✅ 多模型分工节省 50-70% |
| 开源 | ❌ | ✅ MIT 开源 |

## 相关链接

- [MatrixCode CLI](../cli/)
- [MatrixCode GitHub](https://github.com/bigfish1913/matrixcode)
- [工作流使用指南](../../docs/workflow-guide.md)
- [VS Code 扩展开发文档](https://code.visualstudio.com/api)

## 许可证

MIT License