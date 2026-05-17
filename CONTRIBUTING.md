# 贡献指南

感谢你对 MatrixCode 项目的兴趣！

## 项目结构

```
matrixcode/
├── packages/
│   ├── cli/          # Rust CLI 工具
│   └── vscode/       # VSCode 扩展
├── docs/             # 文档
├── .github/          # CI/CD 配置
└── scripts/          # 开发脚本
```

## 开发环境设置

### 前置要求

- Rust 1.70+ (安装: https://rustup.rs)
- Node.js 18+ (安装: https://nodejs.org)
- Git

### 快速开始

```bash
# Linux/macOS
./scripts/setup.sh

# Windows
scripts\setup.bat

# 或手动设置
cd packages/cli
cargo build --release

cd ../vscode
npm install
npm run compile
```

### 配置 API Key

编辑 `packages/cli/.env`:

```env
PROVIDER=anthropic
API_KEY=sk-ant-your-key-here
MODEL_NAME=claude-sonnet-4-20250514
```

## 开发流程

### 1. 创建分支

```bash
git checkout -b feature/your-feature-name
```

### 2. 开发 CLI

```bash
cd packages/cli

# 运行测试
cargo test

# 检查格式
cargo fmt --check

# 运行 clippy
cargo clippy

# 构建
cargo build
```

### 3. 开发 VSCode 扩展

```bash
cd packages/vscode

# 安装依赖
npm install

# 构建
npm run compile

# 代码检查
npm run lint

# 在 VSCode 中调试: 按 F5
```

### 4. 使用 Taskfile

```bash
# 查看所有可用任务
task --list

# 常用任务
task build          # 构建 CLI
task build-vscode   # 构建 VSCode 扩展
task test           # 运行 CLI 测试
task test-vscode    # 运行 VSCode lint
task clean          # 清理构建产物
```

### 5. 提交代码

```bash
# 确保所有测试通过
task test

# 提交
git add .
git commit -m "feat: 描述你的改动"
git push origin feature/your-feature-name
```

### 5. 创建 Pull Request

在 GitHub 上创建 Pull Request，描述你的改动。

## 代码规范

### Rust (CLI)

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 添加适当的文档注释
- 为新功能添加测试

### TypeScript (VSCode 扩展)

- 使用 ESLint 规则
- 使用明确的类型定义
- 添加 JSDoc 注释

## 发布流程

### 版本号规范

遵循语义化版本: `MAJOR.MINOR.PATCH`

- MAJOR: 重大改动
- MINOR: 新功能
- PATCH: Bug 修复

### 发布步骤

```bash
# 使用 Taskfile
task release -- 0.2.6

# 或手动
# 1. 更新版本号
# 2. 创建 tag
# 3. 推送到 GitHub
# 4. CI/CD 自动构建和发布
```

## 问题反馈

在 GitHub Issues 中提交问题：

- 描述问题或建议
- 提供复现步骤（如果是 bug）
- 提供环境信息

## 许可证

MIT License - 贡献的代码将使用相同许可证