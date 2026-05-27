# CLI 项目拆分方案分析

## 方案对比

### 方案 A: 独立仓库（多仓库）

```
仓库结构:
├── matrixcode-core/        # 独立仓库 - CLI/Agent 核心
│   ├── src/
│   │   ├── agent.rs
│   │   ├── providers/
│   │   ├── tools/
│   │   └── event.rs
│   └── Cargo.toml
│
├── matrixcode-tui/         # 独立仓库 - 终端 UI
│   ├── src/
│   │   ├── spinner.rs
│   │   ├── markdown.rs
│   │   └── terminal.rs
│   └ Cargo.toml
│   └── 依赖 matrixcode-core
│
├── matrixcode-vscode/      # 独立仓库 - VSCode 插件
│   ├── src/
│   ├── package.json
│   └── 依赖 matrixcode-core (通过 npm 或本地)
│
└── matrixcode-web/         # 未来：Web UI（可选）
```

**优点**：
- ✅ 各项目独立发布，版本控制清晰
- ✅ 可以独立 CI/CD
- ✅ 其他项目可以引用 matrixcode-core
- ✅ 职责边界清晰

**缺点**：
- ❌ 开发时需要跨仓库协调
- ❌ 事件协议改动需要同步多个仓库
- ❌ 用户需要理解多个项目关系
- ❌ Issue/PR 分散
- ❌ 发布流程复杂（需要协调版本）

---

### 方案 B: Monorepo（单仓库多包）

```
仓库结构:
matrixcode/                 # 单仓库
├── packages/
│   ├── core/               # CLI 核心 crate
│   │   ├── src/
│   │   │   ├── agent.rs
│   │   │   ├── providers/
│   │   │   ├── tools/
│   │   │   └── event.rs
│   │   └ Cargo.toml
│   │   └── 发布到 crates.io
│   │
│   ├── cli/                # CLI 入口（依赖 core + tui）
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── modes.rs
│   │   └ Cargo.toml
│   │   └── 发布到 crates.io / npm
│   │
│   ├── tui/                # 终端 UI crate
│   │   ├── src/
│   │   │   ├── spinner.rs
│   │   │   ├── markdown.rs
│   │   │   └── terminal.rs
│   │   └ Cargo.toml
│   │   └── 内部使用（或单独发布）
│   │
│   └── vscode/             # VSCode 插件
│   │   ├── src/
│   │   ├── package.json
│   │   └── 发布到 VSCode Marketplace
│   │
└── Cargo.workspace         # Rust workspace
└── package.json            # npm workspaces (可选)
```

**优点**：
- ✅ 统一管理，改动同步方便
- ✅ 事件协议一处定义，多处使用
- ✅ 统一 CI/CD
- ✅ 统一 Issue/PR 管理
- ✅ 发布流程可以自动化
- ✅ 开发时不需要跨仓库

**缺点**：
- ❌ 仓库较大
- ❌ 各包版本需要协调
- ❌ CI 时间可能较长（测试所有包）

---

### 方案 C: CLI 主仓库 + UI 子项目（当前方案改进）

```
仓库结构:
matrixcode/                 # CLI 主仓库
├── src/                    # CLI 核心 + 入口
│   ├── agent.rs
│   ├── providers/
│   ├── tools/
│   ├── event.rs            # 事件协议
│   ├── main.rs             # 多模式入口
│   └── tui/                # 内置终端 UI 模块
│       ├── mod.rs
│       ├── spinner.rs
│       └── markdown.rs
│
├── packages/
│   └ vscode/               # VSCode 插件子项目
│   ├── src/
│   └── package.json
│   └── 依赖 CLI (daemon 模式)
│
└── Cargo.toml              # 单一 crate
└── 发布：matrixcode CLI + VSCode 插件
```

**优点**：
- ✅ 简单，改动最小
- ✅ 终端 UI 内置，不需要单独管理
- ✅ CLI 和插件在一个仓库
- ✅ 用户安装简单（一个 CLI + 一个插件）

**缺点**：
- ❌ 终端 UI 和核心耦合在同一 crate
- ❌ 不能单独发布 UI
- ❌ 其他项目难以引用核心逻辑

---

### 方案 D: Core 作为独立 crate + Monorepo

```
仓库结构:
matrixcode/
├── crates/
│   ├── matrixcode-core/    # 核心 crate（可单独发布）
│   │   ├── src/
│   │   │   ├── agent.rs
│   │   │   ├── providers/
│   │   │   ├── tools/
│   │   │   └── event.rs    # 公开 API
│   │   └ Cargo.toml
│   │   └── lib.rs          # 公开 Agent、Event 等
│   │   └── 发布到 crates.io
│   │
│   ├── matrixcode-tui/     # 终端 UI crate
│   │   ├── src/
│   │   └ Cargo.toml
│   │   └── 依赖 matrixcode-core
│   │   └── 可单独发布（可选）
│   │
│   └── matrixcode-cli/     # CLI 入口
│   │   ├── src/
│   │   │   └── main.rs     # 组合 core + tui
│   │   └ Cargo.toml
│   │   └── 依赖 core + tui
│   │   └── 发布到 crates.io / npm
│   │
├── packages/
│   └ vscode/               # VSCode 插件
│   └── 依赖 CLI (daemon 模式)
│
└── Cargo.toml              # Workspace
    [workspace]
    members = ["crates/*"]
```

**优点**：
- ✅ Core 可单独发布，供其他项目使用
- ✅ 统一仓库管理
- ✅ 各 crate 职责清晰
- ✅ 可以独立测试各部分
- ✅ 终端 UI 可以单独演进

**缺点**：
- ❌ 结构稍复杂
- ❌ 需要管理多个 Cargo.toml
- ❌ 版本协调（workspace 版本）

---

## 推荐方案

### 🎯 推荐：方案 D（Core crate + Monorepo）

**理由**：

1. **职责清晰**
   - Core: 纯逻辑，无 UI
   - TUI: 终端 UI
   - CLI: 入口，组合 core + tui
   - VSCode: 插件 UI

2. **可复用**
   - Core 可以被其他项目引用
   - 未来可以有 Web UI、Desktop UI 等

3. **统一管理**
   - 一个仓库，改动同步方便
   - 统一 CI/CD
   - 统一 Issue/PR

4. **发布灵活**
   - Core 可以单独发布到 crates.io
   - CLI 作为用户安装入口
   - VSCode 插件独立发布

---

## 详细目录结构

```
matrixcode/
├── crates/
│   ├── matrixcode-core/           # 核心 Agent crate
│   │   ├── src/
│   │   │   ├── lib.rs             # 公开 API
│   │   │   │   pub mod agent;
│   │   │   │   pub mod event;
│   │   │   │   pub mod provider;
│   │   │   │   pub mod tools;
│   │   │   │
│   │   │   ├── agent.rs           # Agent 核心逻辑
│   │   │   ├── event.rs           # 事件协议定义
│   │   │   ├── providers/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── anthropic.rs
│   │   │   │   └── openai.rs
│   │   │   ├── tools/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── read.rs
│   │   │   │   ├── write.rs
│   │   │   │   ├── edit.rs
│   │   │   │   ├── bash.rs
│   │   │   │   └── ...
│   │   │   ├── session.rs         # 会话管理
│   │   │   ├── memory.rs          # 跨会话记忆
│   │   │   ├── compress.rs        # 上下文压缩
│   │   │   └── models.rs          # 模型配置
│   │   │
│   │   ├── Cargo.toml
│   │   │   [package]
│   │   │   name = "matrixcode-core"
│   │   │   version = "0.3.0"
│   │   │   publish = true         # 发布到 crates.io
│   │   │
│   │   └── README.md
│   │   └── 可以被其他项目引用：
│   │       use matrixcode_core::{Agent, Event};
│   │
│   ├── matrixcode-tui/            # 终端 UI crate
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   │   pub mod spinner;
│   │   │   │   pub mod markdown;
│   │   │   │   pub mod terminal;
│   │   │   │   pub mod tool_display;
│   │   │   │
│   │   │   ├── spinner.rs         # Spinner/进度指示
│   │   │   ├── markdown.rs        # Markdown 渲染
│   │   │   ├── terminal.rs        # 终端交互
│   │   │   ├── tool_display.rs    # Tool 可视化
│   │   │   └── event_handler.rs   # 处理 Core 事件
│   │   │
│   │   ├── Cargo.toml
│   │   │   [package]
│   │   │   name = "matrixcode-tui"
│   │   │   version = "0.3.0"
│   │   │   [dependencies]
│   │   │   matrixcode-core = { path = "../core" }
│   │   │
│   │   └── README.md
│   │   └── 使用：处理 AgentEvent，渲染到终端
│   │
│   └── matrixcode-cli/            # CLI 入口
│   │   ├── src/
│   │   │   ├── main.rs            # 入口，多模式
│   │   │   │   - terminal mode: core + tui
│   │   │   │   - service mode: core only (json)
│   │   │   │   - daemon mode: core only (stdin/stdout)
│   │   │   │
│   │   │   ├── config.rs          # CLI 配置
│   │   │   ├── args.rs            # 参数解析
│   │   │   └── modes/
│   │   │   ├── terminal.rs        # 终端模式
│   │   │   ├── service.rs         # 服务模式
│   │   │   └── daemon.rs          # Daemon 模式
│   │   │
│   │   ├── Cargo.toml
│   │   │   [package]
│   │   │   name = "matrixcode"
│   │   │   version = "0.3.0"
│   │   │   [dependencies]
│   │   │   matrixcode-core = { path = "../core" }
│   │   │   matrixcode-tui = { path = "../tui" }
│   │   │
│   │   └── 发布到 crates.io / npm
│   │   └── 用户安装：cargo install matrixcode
│   │
├── packages/
│   └ vscode/                      # VSCode 插件
│   ├── src/
│   │   ├── extension.ts
│   │   ├── matrixcodeClient.ts    # 调用 CLI daemon
│   │   ├── chatView.ts            # 处理事件，渲染 UI
│   │   └── types.ts               # AgentEvent 类型定义
│   │
│   ├── package.json
│   │   "activationEvents": [...]
│   │   "main": "./dist/extension.js"
│   │
│   └── 发布到 VSCode Marketplace
│   └── 用户安装：VSCode 扩展商店搜索 MatrixCode
│
├── Cargo.toml                     # Workspace 根配置
│   [workspace]
│   members = ["crates/*"]
│   resolver = "2"
│
├── package.json                   # npm workspaces (可选)
│   workspaces: ["packages/*"]
│
├── .github/
│   └ workflows/
│   ├── ci.yml                     # 统一 CI
│   ├── release-core.yml           # Core 发布
│   ├── release-cli.yml            # CLI 发布
│   └── release-vscode.yml         # 插件发布
│
├── docs/
│   ├── ARCHITECTURE_SEPARATION.md
│   ├── CLI_USAGE.md
│   ├── VSCODE_USAGE.md
│   └── EVENT_PROTOCOL.md         # 事件协议文档
│
└── README.md
    └── MatrixCode - AI Code Agent
    └── 安装说明：
        - CLI: cargo install matrixcode
        - VSCode: 扩展商店搜索 MatrixCode
```

---

## 发布策略

### Core crate
```bash
# 发布到 crates.io
cd crates/matrixcode-core
cargo publish

# 其他项目可以引用
# Cargo.toml
[dependencies]
matrixcode-core = "0.3"
```

### CLI
```bash
# 发布到 crates.io
cd crates/matrixcode-cli
cargo publish

# 用户安装
cargo install matrixcode

# 或 npm
npm install -g @bigfishnpm/matrixcode
```

### VSCode 插件
```bash
# 发布到 VSCode Marketplace
cd packages/vscode
vsce publish

# 用户安装
# VSCode 扩展商店搜索 "MatrixCode"
```

---

## 版本协调

### Workspace 版本管理

```toml
# Cargo.toml (workspace 根)
[workspace]
members = ["crates/*"]

# 各 crate Cargo.toml
[dependencies]
matrixcode-core = { path = "../core", version = "0.3" }
```

### 版本更新流程

```bash
# 1. 更新 Core 版本
cd crates/matrixcode-core
# 修改代码
cargo test
cargo publish

# 2. 更新 CLI（依赖新 Core）
cd crates/matrixcode-cli
# Cargo.toml 指向新版本
cargo test
cargo publish

# 3. 更新 VSCode 插件
cd packages/vscode
# package.json 版本号
npm run package
vsce publish
```

---

## CI/CD 配置

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test-core:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd crates/matrixcode-core && cargo test
      
  test-tui:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd crates/matrixcode-tui && cargo test
      
  test-cli:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd crates/matrixcode-cli && cargo test
      
  test-vscode:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd packages/vscode && npm install && npm test
```

---

## 迁移步骤

### Step 1: 创建 Workspace 结构
```bash
mkdir -p crates/matrixcode-core/src
mkdir -p crates/matrixcode-tui/src
mkdir -p crates/matrixcode-cli/src

# 创建 Cargo.toml (workspace)
```

### Step 2: 迁移 Core
```bash
# 移动核心代码到 crates/matrixcode-core/
mv src/agent.rs crates/matrixcode-core/src/
mv src/providers crates/matrixcode-core/src/
mv src/tools crates/matrixcode-core/src/
mv src/event.rs crates/matrixcode-core/src/  # 新文件
mv src/session.rs crates/matrixcode-core/src/
mv src/memory.rs crates/matrixcode-core/src/
mv src/compress.rs crates/matrixcode-core/src/
mv src/models.rs crates/matrixcode-core/src/

# 创建 lib.rs (公开 API)
```

### Step 3: 迁移 TUI
```bash
# 移动 UI 代码到 crates/matrixcode-tui/
mv src/ui.rs crates/matrixcode-tui/src/terminal.rs
mv src/markdown.rs crates/matrixcode-tui/src/
mv src/tools/spinner.rs crates/matrixcode-tui/src/spinner.rs

# 创建 lib.rs
```

### Step 4: 创建 CLI 入口
```bash
# crates/matrixcode-cli/src/main.rs
# 组合 core + tui
```

### Step 5: 更新 VSCode 插件
```bash
# packages/vscode/src/types.ts
# 定义 AgentEvent 类型（从 Core 的 event.rs）

# packages/vscode/src/matrixcodeClient.ts
# 使用 daemon 模式
```

---

## 总结

### 推荐：方案 D（Core crate + Monorepo）

| 方面 | 说明 |
|------|------|
| 结构 | crates/core + crates/tui + crates/cli + packages/vscode |
| 管理 | 统一仓库，统一 CI/CD |
| 发布 | Core 可单独发布，CLI 和插件独立发布 |
| 复用 | Core 可被其他项目引用 |
| 开发 | 改动同步方便，事件协议一处定义 |

### 迁移时间估计

| Step | 内容 | 时间 |
|------|------|------|
| 1 | 创建 Workspace | 0.5 天 |
| 2 | 迁移 Core | 1 天 |
| 3 | 迁移 TUI | 1 天 |
| 4 | 创建 CLI 入口 | 1 天 |
| 5 | 更新 VSCode 插件 | 1 天 |
| 6 | CI/CD 配置 | 0.5 天 |

**总计**: 约 5 天

---

## 是否拆分？

### ✅ 建议拆分（方案 D）

**理由**：
1. 职责清晰，Core 纯逻辑，UI 纯渲染
2. Core 可复用，未来可扩展其他 UI
3. 统一管理，开发方便
4. 灵活发布，各部分独立版本

### ❌ 不建议完全独立仓库

**理由**：
1. CLI 和 UI 紧密相关，需要同步改动
2. 事件协议需要一处定义
3. 跨仓库协调成本高
4. 用户理解成本高