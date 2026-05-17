# MatrixCode

智能 AI 代码代理工具集，包含 CLI 和 VSCode 扩展。

## 项目结构

```
matrixcode/
├── packages/
│   ├── cli/          # Rust CLI 工具
│   │   ├── src/      # 源代码
│   │   ├── tests/    # 测试
│   │   ├── npm/      # npm 发布包
│   │   └── Cargo.toml
│   │
│   └── vscode/       # VSCode 扩展
│       ├── src/      # TypeScript 源码
│       ├── package.json
│       └── README.md
│
├── docs/             # 共享文档
├── .github/          # CI/CD 配置
└── README.md         # 本文件
```

## 子项目

### 📦 packages/cli

智能代码代理 CLI 工具，核心功能：

- 🤖 多模型配置 (main/plan/compress/fast)
- 🧠 跨会话记忆系统
- 🗜️ 智能上下文压缩
- 📋 任务规划与分解
- 💾 会话持久化管理
- 📁 文件操作工具
- 🌐 Web 搜索能力
- 🔧 可扩展技能系统

[→ 查看 CLI 项目](packages/cli/)

### 📦 packages/vscode

VSCode 扩展，侧边栏 AI 助手：

- 💬 侧边栏聊天界面
- ⚡ 代码快速操作 (解释/修复/重构/测试)
- 📍 自动上下文附加
- 🎯 流式响应渲染
- ⚙️ VSCode 设置集成

[→ 查看 VSCode 扩展](packages/vscode/)

## 快速开始

### 安装 CLI

```bash
# 通过 npm（推荐）
npm install -g matrixcode

# 通过 Cargo
cargo install matrixcode
```

### 配置 API Key

```bash
# 创建配置文件
mkdir -p ~/.matrix
cat > ~/.matrix/config.json << 'EOF'
{
  "apiKey": "your-api-key-here"
}
EOF

# 或设置环境变量
export ANTHROPIC_API_KEY=your-key
```

### 使用 CLI

```bash
# 交互模式
matrixcode

# 单次问答
matrixcode "分析这个项目的结构"

# JSON 模式（VSCode 集成）
matrixcode --json "你的问题"
```

### 开发 VSCode 扩展

```bash
cd packages/vscode
npm install
npm run compile

# 在 VSCode 中按 F5 启动调试
```

## 开发命令

```bash
# 构建 CLI
cd packages/cli && cargo build --release

# 构建 VSCode 扩展
cd packages/vscode && npm run compile

# 运行测试
cd packages/cli && cargo test

# 发布 CLI
cargo publish && cd npm && npm publish

# 发布 VSCode 扩展
cd packages/vscode && npm run publish
```

### 使用 Taskfile

```bash
# 安装 task (如果未安装)
# macOS/Linux: brew install go-task/tap/go-task
# Windows: scoop install task

# 构建所有
task build

# 构建 CLI
task build-cli

# 构建 VSCode 扩展
task build-vscode

# 运行测试
task test

# 清理
task clean

# 发布 CLI + VSCode 扩展（自动升级版本）
task publish

# 发布指定版本
task release -- 0.2.6

# 仅发布 VSCode 扩展
task publish-vscode

# 查看所有任务
task --list
```

## 文档

- [CLI 使用指南](docs/)
- [VSCode 插件设计](docs/VSCode_Plugin_Design.md)
- [CI/CD 说明](docs/CI_CD.md)

## 许可证

MIT License

## 链接

- [GitHub](https://github.com/bigfish1913/matrixcode)
- [问题反馈](https://github.com/bigfish1913/matrixcode/issues)