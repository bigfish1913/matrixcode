# Todo 死循环问题修复报告

## 问题分析

### 死循环场景

在使用其他模型时，todo 处理逻辑会出现死循环，导致会话一直卡在第一个 todo 上无法完成。

**死循环流程：**

1. 模型创建 todo 列表（任务状态：`in_progress` 或 `pending`）
2. 模型执行操作后返回 `should_continue = false`（想要停止）
3. **关键问题**：系统检测到未完成的 todo，添加提醒消息，强制 `should_continue = true`
4. 其他模型可能：
   - 不理解如何更新 todo（缺少相关训练）
   - 执行其他操作但没更新 todo 状态
   - 再次返回 `should_continue = false`
5. 系统再次检测到**相同的未完成 todo**，再次添加提醒
6. **无限循环** ♻️

### 根本原因

**问题代码位置：**

- `core/src/agent/helpers.rs:64` - `get_pending_todos()` 只检查最近的 todo_write
- `core/src/agent/run.rs:319-344` - 没有防止重复提醒的机制

**核心问题：**

```rust
// helpers.rs:64
pub(crate) fn get_pending_todos(&self) -> Vec<(String, String)> {
    // 找到第一个 todo_write 就返回，不检查是否已提醒过
}

// run.rs:319-344
let pending = self.get_pending_todos();
if !pending.is_empty() {
    // 总是会添加提醒，不管是否已经提醒过相同内容
    self.messages.push(Message {
        role: Role::User,
        content: MessageContent::Text(reminder),
    });
    should_continue = true; // 强制继续
}
```

## 解决方案

采用**组合方案**：提醒计数器 + 重复检查 + 上限限制

### 修改内容

#### 1. 添加 todo 提醒计数器

**文件：** `core/src/agent/types.rs`

```rust
use std::collections::{HashMap, HashSet}; // 添加 HashMap

pub struct Agent {
    // ... 其他字段
    pub(crate) todo_reminder_count: HashMap<String, usize>, // 新增
}
```

#### 2. 新增智能 todo 检查方法

**文件：** `core/src/agent/helpers.rs`

```rust
/// 获取未完成的 todo，排除已达到提醒上限的
pub(crate) fn get_pending_todos_with_limit(
    &self,
    todo_reminder_count: &HashMap<String, usize>,
    max_reminders: usize, // 默认值：2
) -> (Vec<(String, String)>, bool) {
    // 返回：(未完成且未达上限的 todo, 是否所有 todo 都已达上限)
}

/// 检查最近是否已发送过 todo 提醒
pub(crate) fn last_message_was_todo_reminder(&self) -> bool {
    // 检查最近 3 条消息中是否包含 todo 提醒
}
```

#### 3. 修改 todo 检查逻辑

**文件：** `core/src/agent/run.rs`

```rust
// 旧逻辑：直接检查并强制继续
let pending = self.get_pending_todos();
if !pending.is_empty() {
    // 添加提醒 + 强制继续
}

// 新逻辑：三层防护
if self.last_message_was_todo_reminder() {
    // 第一层：跳过（最近已提醒）
} else {
    const MAX_TODO_REMINDERS: usize = 2;
    let reminder_count_clone = self.todo_reminder_count.clone();
    let (pending, all_at_limit) = self.get_pending_todos_with_limit(
        &reminder_count_clone,
        MAX_TODO_REMINDERS
    );
    
    if !pending.is_empty() {
        // 第二层：更新计数器 + 添加提醒
        for (_, content) in &pending {
            *self.todo_reminder_count.entry(content.clone()).or_insert(0) += 1;
        }
        // 添加提醒 + 强制继续
    } else if all_at_limit && !self.todo_reminder_count.is_empty() {
        // 第三层：所有 todo 都已达上限，允许会话结束
        // 提示用户：N 个待办项未完成（已提醒 2 次，达到上限）
    }
}
```

### 三层防护机制

| 层级 | 检查内容 | 效果 |
|------|---------|------|
| **第1层** | `last_message_was_todo_reminder()` | 跳过最近已提醒的 todo（防止连续重复） |
| **第2层** | `get_pending_todos_with_limit()` + 计数器更新 | 每个 todo 最多提醒 2 次 |
| **第3层** | `all_at_limit` 检查 | 所有 todo 都达上限后，允许会话结束 |

## 测试验证

### 编译检查

```bash
cd core && cargo check
# ✅ Finished successfully
```

### 单元测试

```bash
cd core && cargo test --lib todo
# ✅ test_task_plan_to_todo ... ok
```

### 预期行为

#### 正常场景（模型理解 todo）

```
iteration 1: 创建 todo（任务 A：pending）
iteration 2: 执行任务 A，更新 todo（任务 A：completed）
iteration 3: 无未完成 todo → 正常结束
```

#### 异常场景（模型不理解 todo）

**旧逻辑（死循环）：**

```
iteration 1: 创建 todo（任务 A：pending）
iteration 2: 模型返回停止 → 提醒任务 A → 强制继续
iteration 3: 模型返回停止 → 提醒任务 A → 强制继续
iteration 4: 模型返回停止 → 提醒任务 A → 强制继续
... 无限循环 ...
```

**新逻辑（安全结束）：**

```
iteration 1: 创建 todo（任务 A：pending）
iteration 2: 模型返回停止 → 提醒任务 A（计数=1）→ 强制继续
iteration 3: 模型返回停止 → 提醒任务 A（计数=2）→ 强制继续
iteration 4: 模型返回停止 → 计数器达到上限 → 允许结束
           → 提示用户："1 个待办项未完成（已提醒 2 次，达到上限）"
```

## 影响范围

### 修改文件

- `core/src/agent/types.rs` - 新增 `todo_reminder_count` 字段
- `core/src/agent/helpers.rs` - 新增智能 todo 检查方法
- `core/src/agent/run.rs` - 修改 todo 检查逻辑

### 兼容性

- ✅ ��持向后兼容（保留 `get_pending_todos()` 方法）
- ✅ 不影响现有 todo_write 工具行为
- ✅ 不影响其他模型正常处理 todo 的能力

## 总结

通过**三层防护机制**有效防止 todo 死循环：

1. **提醒计数器**：每个 todo 最多提醒 2 次
2. **重复检查**：跳过最近已提醒的 todo
3. **上限限制**：所有 todo 都达上限后，允许会话结束

这样既保证了模型有足够机会处理 todo（最多 2 次提醒），又避免了无限循环导致的资源浪费和用户体验问题。