# CLI 项目拆分进度

## Phase 1: 基础架构 ✅ 完成

### 已完成的工作

1. **创建 Workspace 结构**
   - packages/cli/Cargo.toml (workspace 配置)
   - crates/matrixcode-core/
   - crates/matrixcode-tui/
   - crates/matrixcode-cli/

2. **定义事件协议**
   - AgentEvent 结构
   - EventType 枚举 (20+ 事件类型)
   - EventData 枚举 (文本、Tool、错误等)
   - EventCollector 收集器
   - JSON 序列化/反序列化

3. **创建 Core crate**
   - lib.rs: 公开 API
   - event.rs: 事件定义
   - 测试通过

4. **创建 TUI crate**
   - lib.rs: TerminalUI 处理器
   - 事件渲染逻辑
   - 测试通过

5. **创建 CLI 入口**
   - main.rs: 多模式支持
   - terminal/service/daemon 模式
   - 基础命令: chat, quick-action, new-session

6. **编译测试**
   - cargo build --release ✅
   - cargo test ✅ (3 tests passed)
   - CLI 运行验证 ✅

### CLI 测试结果

```bash
$ matrixcode --help
AI Code Agent with multi-model support

Commands:
  chat          启动聊天会话
  quick-action  快速操作
  new-session   创建新会话
  history       显示会话历史
  status        显示状态

Options:
  -m, --mode <MODE>  运行模式 [default: terminal]

$ matrixcode chat --message "Hello"
--- Session Started ---
Processing: Hello
--- Session Ended ---
```

### 项目结构

```
packages/cli/
├── Cargo.toml              # Workspace
├── crates/
│   ├── matrixcode-core/    # ✅ 核心事件协议
│   ├── matrixcode-tui/     # ✅ Terminal UI
│   └── matrixcode-cli/     # ✅ CLI 入口
├── src/                    # 旧代码（待迁移）
│   ├── agent.rs            # → Core
│   ├── providers/          # → Core
│   ├── tools/              # → Core
│   ├── session.rs          # → Core
│   ├── memory.rs           # → Core
│   ├── compress.rs         # → Core
│   ├── ui.rs               # → TUI
│   └── markdown.rs         # → TUI
└── target/release/matrixcode.exe
```

---

## Phase 2: 核心迁移 (待完成)

### 待迁移的模块

| 模块 | 来源 | 目标 | 优先级 |
|------|------|------|--------|
| agent.rs | src/ | Core | P1 |
| providers/ | src/ | Core | P1 |
| tools/ | src/ | Core | P1 |
| session.rs | src/ | Core | P2 |
| memory.rs | src/ | Core | P2 |
| compress.rs | src/ | Core | P2 |
| models.rs | src/ | Core | P2 |
| config.rs | src/ | Core | P2 |
| ui.rs | src/ | TUI | P2 |
| markdown.rs | src/ | TUI | P2 |

### 迁移步骤

1. **迁移 agent.rs**
   - 移除 UI 代码
   - ���生 AgentEvent
   - 发布为 Core API

2. **迁移 providers/**
   - 保持不变
   - 只改输出格式

3. **迁移 tools/**
   - 移除 spinner
   - 产生 ToolUse/ToolResult 事件

4. **迁移 TUI**
   - 移动 ui.rs
   - 移动 markdown.rs
   - 添加 spinner

---

## Phase 3: VSCode 插件适配 (待完成)

### 改动点

1. **matrixcodeClient.ts**
   - 使用 daemon 模式
   - 监听 stdout JSON 流
   - 处理 AgentEvent

2. **chatView.ts**
   - 处理结构化事件
   - 渲染 Tool Use 卡片
   - 显示进度

3. **types.ts**
   - 定义 TypeScript 事件类型

---

## 架构优势

### 当前架构 (拆分后)

```
┌─────────────────────────────────────┐
│           UI Layer                   │
│  ┌─────────────┐  ┌──────────────┐  │
│  │ VSCode 插件 │  │  Terminal UI │  │
│  │ - 动画      │  │ - spinner    │  │
│  │ - 渲染      │  │ - markdown   │  │
│  └─────────────┘  └──────────────┘  │
└─────────────────────────────────────┘
         ↕ (JSON 事件流)
┌─────────────────────────────────────┐
│        CLI Core                      │
│  - Agent 核心逻辑                    │
│  - Provider 调用                     │
│  - Tool 执行                         │
│  - 只输出 AgentEvent                │
└─────────────────────────────────────┘
```

### 优势对比

| 方面 | 拆分前 | 拆分后 |
|------|--------|--------|
| 职责 | CLI 混杂 UI | Core 纯逻辑，TUI 纯渲染 |
| 通信 | 复杂 | 简单 JSON 流 |
| 测试 | 困难 | 事件是纯数据，易测试 |
| 扩展 | 难 | 新 UI 只需处理事件 |
| 发布 | 单一 | Core 可单独发布 |

---

## 下次继续

```bash
cd packages/cli

# 继续迁移 Agent
# 1. 移动 src/agent.rs → crates/matrixcode-core/src/agent.rs
# 2. 修改 agent 产生 AgentEvent
# 3. 测试编译
```