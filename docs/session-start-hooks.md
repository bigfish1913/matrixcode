# SessionStart Hooks 使用指南

## 概述

SessionStart hooks 是在用户发送第一条消息之前动态注入提示词内容的机制。

## 注入顺序

```
用户会话开始:
──────────────────────────────────────────>

1. CLI 启动
   ├── 核心系统提示词 (静态)
   ├── 工具 Schema 定义
   └── 环境信息

2. SessionStart Hook 注入
   ├── 强制技能警告
   ├── 红旗警告表格
   └── 技能优先级规则

3. TODO 提醒 (如果有待办)
   └── pending/in_progress 任务

4. 诊断信息 (如果有)
   └── LSP/rustc 错误和警告

5. 用户消息处理
```

## 使用方法

### 1. SessionStartHook - 会话开始注入

```rust
use matrixcode_core::prompt::SessionStartHook;

// 创建 hook
let hook = SessionStartHook::new()
    .add_mandatory_skill("code-review")
    .add_mandatory_skill("debug")
    .with_red_flags(true)
    .with_skill_priority(true);

// 生成注入内容
let content = hook.build();
// 输出:
// <EXTREMELY-IMPORTANT>
// The following skills are **MANDATORY**...
// </EXTREMELY-IMPORTANT>
//
// ## Red Flags - STOP and reconsider...
//
// ## Skill Priority...
```

### 2. TodoReminder - TODO 提醒

```rust
use matrixcode_core::prompt::TodoReminder;

// 创建提醒
let reminder = TodoReminder::new()
    .set_in_progress("正在实现功能 A")
    .set_pending_tasks(vec![
        "运行测试".to_string(),
        "代码审查".to_string(),
    ])
    .with_max_reminders(2);  // 每个任务最多提醒2次

// 生成提醒内容
if let Some(content) = reminder.build() {
    // 输出:
    // <todo-reminder>
    // ⏳ **In Progress**: 正在实现功能 A
    //
    // 📋 **Pending Tasks**:
    //   - 运行测试
    //   - 代码审查
    // </todo-reminder>
}

// 检查是否应该提醒
if reminder.should_remind("运行测试") {
    reminder.increment_reminder("运行测试");
}
```

### 3. DiagnosticsInjection - 诊断注入

```rust
use matrixcode_core::prompt::{DiagnosticsInjection, DiagnosticEntry};

// 创建诊断注入
let injection = DiagnosticsInjection::new()
    .add_diagnostic(DiagnosticEntry {
        file: "src/main.rs".to_string(),
        line: 42,
        severity: "error".to_string(),
        message: "missing semicolon".to_string(),
        source: "rustc".to_string(),
    })
    .add_diagnostic(DiagnosticEntry {
        file: "src/lib.rs".to_string(),
        line: 10,
        severity: "warning".to_string(),
        message: "unused variable".to_string(),
        source: "rust-analyzer".to_string(),
    })
    .with_max_entries(20);

// 生成诊断内容
if let Some(content) = injection.build() {
    // 输出:
    // <new-diagnostics>
    // The following new diagnostic issues were detected:
    //
    // ✘ src/main.rs:42 missing semicolon [rustc]
    // ⚠ src/lib.rs:10 unused variable [rust-analyzer]
    //
    // </new-diagnostics>
}

// 检查状态
if injection.has_errors() {
    // 有编译错误
}
```

### 4. SessionStartContext - 组合使用

```rust
use matrixcode_core::prompt::SessionStartContext;

// 创建完整上下文
let context = SessionStartContext {
    hook: SessionStartHook::new()
        .add_mandatory_skill("code-review"),
    todo: TodoReminder::new()
        .set_pending_tasks(vec!["完成任务".to_string()]),
    diagnostics: DiagnosticsInjection::new(),
};

// 生成完整注入内容
let content = context.build();
// 输出所有动态注入内容
```

## 集成到 Agent

在 Agent 会话开始时注入：

```rust
// 在 agent/run.rs 或 agent/session/manager.rs 中

pub async fn run(&mut self) -> Result<()> {
    // 1. 发送 SessionStarted 事件
    self.emit(AgentEvent::session_started())?;

    // 2. 构建动态注入内容
    let session_context = SessionStartContext {
        hook: self.build_session_start_hook(),
        todo: self.build_todo_reminder(),
        diagnostics: self.collect_diagnostics(),
    };

    // 3. 如果有内容，添加到系统提示词
    if session_context.has_content() {
        let dynamic_content = session_context.build();
        // 添加到 messages 或直接注入到系统提示词
    }

    // 4. 开始处理用户消息
    ...
}
```

## 效果对比

### Claude Code 提示词结构

```
You are Claude Code, Anthropic's official CLI...

SessionStart hook additional context:
<EXTREMELY-IMPORTANT>...</EXTREMELY-IMPORTANT>

The following deferred tools are now available...

The following skills are available for use...

<todo-reminder>
⏳ In Progress: implementing feature
</todo-reminder>

<new-diagnostics>
✘ src/main.rs:42 error [rustc]
</new-diagnostics>
```

### MatrixCode 改进后结构

```
你是 MatrixCode - 基于 Rust 的智能代码助手...

SessionStart hook additional context:
<EXTREMELY-IMPORTANT>
code-review skill is **MANDATORY**
</EXTREMELY-IMPORTANT>

## Red Flags - STOP and reconsider...

## Skill Priority...

<todo-reminder>
⏳ 正在实现功能 A
📋 Pending: 运行测试
</todo-reminder>

<new-diagnostics>
✘ src/main.rs:42 missing semicolon [rustc]
</new-diagnostics>
```

## 后续工作

1. **与 LSP 集成**: 实时收集诊断信息
2. **与 TodoWrite 集成**: 自动跟踪待办状态
3. **与 Skills 系统集成**: 自动检测强制技能
4. **缓存优化**: 静态内容缓存，动态内容刷新