# MatrixCode

AI 智能代码代理工具集，提供 CLI 和 VS Code 扩展两种使用方式。

## 项目概述

MatrixCode 是一个多模型 AI 代码助手，支持：
- 多模型配置 (main/plan/compress/fast)
- 跨会话记忆系统
- 智能上下文压缩
- 任务规划与分解
- 会话持久化管理
- 文件操作工具
- Web 搜索能力
- 可扩展技能系统

## 项目结构

```
matrixcode/
├── packages/
│   ├── cli/                    # Rust CLI 工具
│   │   ├── crates/
│   │   │   ├── matrixcode-core/    # 核心逻辑库 (无 UI 依赖)
│   │   │   ├── matrixcode-tui/     # TUI 界面库 (ratatui)
│   │   │   └── matrixcode-cli/     # CLI 入口
│   │   ├── Cargo.toml
│   │   └── tests/
│   │
│   └── vscode/                 # VS Code 扩展 (TypeScript)
│       ├── src/                    # TypeScript 源码
│       ├── package.json
│       └── dist/
│
├── skills/                     # 技能文件目录
├── docs/                       # 文档目录
├── scripts/                    # 构建脚本
├── .matrix/                    # 本地配置目录
├── Taskfile.yml                # Task 任务定义
├── README.md
├── CHANGELOG.md
├── CONTRIBUTING.md
└── LICENSE
```

## 技术栈

### CLI (Rust)
- **Rust 2024 Edition**
- **ratatui**: Terminal UI 框架
- **crossterm**: 跨平台终端控制
- **tokio**: 异步运行时
- **reqwest**: HTTP 客户端 (API 调用)
- **serde/serde_json**: 序列化
- **pulldown-cmark**: Markdown 解析
- **syntect**: 语法高亮

### VS Code 扩展 (TypeScript)
- **TypeScript 5.3+**
- **esbuild**: 打包构建
- **VS Code Extension API**

## 开发规范

### 代码风格

#### Rust
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 为新功能添加测试
- 添加适当的文档注释

#### TypeScript
- 使用 ESLint 规则
- 使用明确的类型定义
- 添加 JSDoc 注释

### 提交规范 (Conventional Commits)

```
<type>(<scope>): <description>

# 类型:
feat     - 新功能
fix      - Bug 修复
docs     - 文档更新
style    - 代码格式 (不影响功能)
refactor - 重构
test     - 测试相关
chore    - 构建/工具相关
perf     - 性能优化
```

示例:
```
feat(tui): add streaming response support
fix(core): resolve memory leak in session handling
docs: update API configuration guide
```

### 分支策略

- `master` - 主分支，稳定版本
- `dev` - 开发分支
- `feature/*` - 功能分支
- `fix/*` - 修复分支

## 构建与测试

### Taskfile 常用命令

```bash
# 查看所有任务
task --list

# 构建
task build          # 构建 CLI (release)
task dev            # 构建 CLI (debug)
task build-vscode   # 构建 VS Code 扩展

# 测试
task test           # 运行所有 CLI 测试
task test-core      # 运行 core 模块测试
task test-tui       # 运行 TUI 模块测试
task test-vscode    # 运行 VS Code lint

# 代码质量
task check          # clippy + fmt 检查
task fmt            # 格式化代码

# 安装与运行
task install        # 本地安装 CLI
task run            # 运行 CLI

# 发布
task publish        # 自动升级版本并发布 (cargo + vscode)
task release -- 0.x.x  # 发布指定版本
```

### 手动命令

```bash
# CLI (Rust)
cd packages/cli
cargo build --release
cargo test --all
cargo clippy --all -- -D warnings
cargo fmt --all

# VS Code 扩展 (TypeScript)
cd packages/vscode
npm install
npm run compile
npm run lint
```

### 配置 API Key

编辑 `packages/cli/.env` 或 `~/.matrix/config.json`:

```env
PROVIDER=anthropic
API_KEY=sk-ant-your-key-here
MODEL_NAME=claude-sonnet-4-20250514
```

## 发布流程

### 版本号规范

遵循语义化版本: `MAJOR.MINOR.PATCH`

### 自动发布

```bash
task publish  # 自动升级 patch 版本并发布
```

### 手动发布

```bash
# 1. 更新版本号
task bump -- 0.4.2

# 2. 创建 tag 并推送
task tag -- 0.4.2

# 3. 发布到 cargo
task publish-cargo

# 4. 发布到 VS Code Marketplace
task publish-vscode
```

## 相关链接

- [GitHub](https://github.com/bigfish1913/matrixcode)
- [问题反馈](https://github.com/bigfish1913/matrixcode/issues)