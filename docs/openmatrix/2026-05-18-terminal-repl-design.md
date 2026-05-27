# 设计方案: Terminal REPL 模式实现

日期: 2026-05-18

## 核心目标

- 全功能 TUI 界面（状态栏、输出区、输入框、侧边面板）
- 内置命令支持（/help, /exit, /clear, /model, /session）
- 历史导航（上下箭头浏览输入历史）
- 中断处理（Ctrl+C 取消当前请求）
- 会话持久化（保存/恢复会话）

## 架构设计

### 整体架构

```
matrixcode-tui (重构)
├─ app.rs          → App struct (ratatui 主应用)
├─ components/     → UI 组件
│   ├─ status_bar.rs    → 状态栏
│   ├─ output_area.rs   → 输出区域
│   ├─ input_box.rs     → 输入框
│   └─ side_panel.rs    → 侧边面板
├─ handler.rs      → 输入事件处理
├─ bridge.rs       → AgentEvent → UI 状态转换
└─ session.rs      → 会话持久化

matrixcode-cli (修改)
└─ main.rs
    └─ run_terminal_mode()
        ├─ 创建 tokio runtime
        ├─ 启动 Agent 任务
        ├─ 创建 channel 连接
        └─ 运行 ratatui App

matrixcode-core (不变)
└─ Agent.run() → mpsc::Sender<AgentEvent>
```

### 数据流

```
用户输入 → InputBox
    ↓
InputHandler 处理
    ↓ (如果是普通消息)
发送到 Agent (tokio task)
    ↓
Agent.run() 异步执行
    ↓
AgentEvent → mpsc::channel
    ↓
EventBridge 接收
    ↓
更新 AppState
    ↓
ratatui 重绘
```

## UI 组件设计

### StatusBar（状态栏）

```
┌────────────────────────────────────────────────────────┐
│ MatrixCode 0.3.0 | Model: claude-sonnet-4.6 | 1.2k tokens │
└────────────────────────────────────────────────────────┘
```

- 显示：版本、当前模型、token 使用量、会话状态
- 颜色：蓝色背景，白色文字

### OutputArea（输出区）

```
┌────────────────────────────────────────────────────────┐
│ User: 请帮我实现 REPL                                   │
│                                                        │
│ Assistant: 好的，我来帮你...                            │
│   [Tool: read_file] → Result: ...                     │
│   [Tool: edit_file] → Result: Done                    │
│                                                        │
│ ⏳ Thinking...                                         │
└────────────────────────────────────────────────────────┘
```

- 可滚动查看历史
- Markdown 渲染（简化版）
- 工具执行状态可视化

### InputBox（输入框）

```
┌────────────────────────────────────────────────────────┐
│ > 请帮我检查代码_                                       │
│   [Enter: 发送] [Esc: 取消] [↑↓: 历史] [Ctrl+C: 中断]  │
└────────────────────────────────────────────────────────┘
```

- 多行输入支持
- 命令前缀 `/` 高亮
- 输入历史存储（最近 100 条）

### SidePanel（侧边面板）

```
┌──────────────────┐
│ 📁 Tools         │
│  ├─ read         │
│  ├─ write        │
│  ├─ edit         │
│  ├─ bash         │
│                  │
│ 🎯 Skills        │
│  ├─ /om:plan     │
│  ├─ /om:start    │
│                  │
│ ⌨️ Commands      │
│  /help, /exit    │
│  /clear, /model  │
└──────────────────┘
```

- 可折叠（Tab 切换）
- 工具/技能快捷参考

## 关键接口 / API

### 输入事件处理

```rust
enum InputAction {
    Send(String),           // 发送消息给 Agent
    Command(Command),       // 执行内置命令
    HistoryUp,              // 上一条历史
    HistoryDown,            // 下一条历史
    Interrupt,              // 中断当前请求
    TogglePanel,            // 切换侧边面板
    ScrollUp,               // 输出区向上滚动
    ScrollDown,             // 输出区向下滚动
}

enum Command {
    Help,                   // 显示帮助
    Exit,                   // 退出 REPL
    Clear,                  // 清空输出
    Model(String),          // 切换模型
    Session(String),        // 会话操作
}
```

### AgentEvent → UI 状态转换

```rust
impl EventBridge {
    fn apply(&mut self, event: AgentEvent, state: &mut AppState) {
        match event.event_type {
            EventType::TextDelta => state.add_output(event.text),
            EventType::ToolUseStart => state.show_tool_start(event.name),
            EventType::ToolResult => state.show_tool_result(event.result),
            EventType::ThinkingDelta => state.show_thinking(event.thinking),
            EventType::SessionEnded => state.set_idle(),
            EventType::Error => state.show_error(event.message),
            ...
        }
    }
}
```

### 中断机制

```rust
// Agent 持有 CancellationToken
let token = CancellationToken::new();
agent.set_cancel_token(token.clone());

// Ctrl+C 时
token.cancel();
// Agent 检测到取消，发送 Error 事件并退出循环
```

## 技术方案

- **TUI 框架**: ratatui（流行的 Rust TUI 库，组件化设计）
- **异步处理**: tokio + channel（主线程 TUI，后台 Agent）
- **中断机制**: CancellationToken
- **存储格式**: JSON 文件

方案选择理由: 分层架构清晰，易测试，可独立演进

## 会话持久化

### 存储结构

```rust
struct SessionData {
    id: String,              // UUID
    created_at: DateTime,
    updated_at: DateTime,
    messages: Vec<Message>,  // Agent 的消息历史
    input_history: Vec<String>, // 用户输入历史
    model: String,           // 当前模型
    project_path: String,    // 项目路径
}
```

### 存储位置

```
~/.matrixcode/sessions/
    ├─ session-{uuid}.json
    └─ latest.json          // 最后一个会话的引用
```

### 持久化时机

- 自动保存: 每次对话结束时
- 手动保存: `/session save` 命令
- 加载: `--continue` 或 `--resume <id>` 参数

### 会话管理命令

```
/session list      → 列出所有会话
/session save      → 手动保存当前会话
/session load <id> → 加载指定会话
/session delete <id> → 删除会话
/session new       → 开始新会话
```

## 错误处理策略

- 输入解析错误: 显示提示信息，不中断 REPL
- Agent 执行错误: 显示错误事件，允许继续对话
- 会话文件损坏: 警告用户，提供恢复选项
- 终端兼容性问题: 使用 ratatui 兼容层，降级为简单模式

## 测试策略

| 层级 | 测试类型 | 覆盖范围 |
|------|---------|---------|
| matrixcode-tui | 单元测试 | 各组件独立渲染、EventBridge 转换逻辑 |
| matrixcode-tui | 集成测试 | App 状态流转、输入处理 |
| matrixcode-cli | 集成测试 | terminal 模式启动、会话加载 |
| 全链路 | 手动测试 | 完整 REPL 交互体验 |

关键测试点:

- EventBridge::apply() - 确保 AgentEvent 正确转换为 UI 状态
- InputHandler::parse() - 命令解析、历史导航
- SessionStore::save/load - 会话持久化正确性
- 中断处理 - CancellationToken 正确取消 Agent

## 约束与风险

### 约束

- 必须使用 ratatui（已确认）
- Agent 已有异步实现，需要适配
- 会话存储格式需要兼容现有 daemon 模式

### 风险及应对

| 风险 | 影响 | 应对策略 |
|------|------|---------|
| ratatui 学习曲线 | 开发时间增加 | 参考官方示例，使用 template 项目 |
| 异步与 TUI 线程协调 | 复杂度高 | 使用 tick 机制定期检查 channel |
| Markdown 渲染性能 | 大量输出时卡顿 | 简化渲染，缓存格式化结果 |
| 终端兼容性 | 不同终端表现不一 | 使用 ratatui 的兼容层，测试主流终端 |
| 会话文件损坏 | 数据丢失 | 写入前备份，使用 JSON 校验 |

## 验收标准

- `matrixcode --mode terminal` 正常启动 REPL
- 支持基本对话和工具执行
- 内置命令全部可用（/help, /exit, /clear, /model, /session）
- Ctrl+C 正确中断请求
- 会话可保存和恢复（--continue, --resume）
- 输入历史可导航（上下箭头）
- 侧边面板可切换显示（Tab）

## 实现顺序

### Phase 1: 基础框架

- 添加 ratatui 依赖
- 创建 App struct 和基础组件
- 实现事件循环
- 集成 Agent (tokio + channel)

### Phase 2: 功能完善

- 输入处理 (历史、命令)
- 中断机制
- 会话持久化
- 侧边面板

### Phase 3: 优化与测试

- Markdown 渲染优化
- 添加单元测试
- 手动测试各功能