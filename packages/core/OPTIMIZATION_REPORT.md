# MatrixCode 提示词优化报告

## 📋 优化概述

成功实现了动态提示词构建系统，根据是否有 CodeGraph 自动选择最合适的工具选择决策链和调试策略版本。

## ✅ 完成的关键改进

### 1. **工具选择决策链** - 动态化

#### 之前（单一版本）
```
- 找代码符号？ → 查看工具列表中的符号搜索工具（如有）
- 查调用关系？ → 查看工具列表中的调用分析工具（如有）
```

#### 现在（两个专门版本）

**有 CodeGraph 时**：
```
- 找代码定义？ → code_search（优先，比 grep 快 10-100 倍）✨
- 查调用关系？ → code_callers/callees（优先，比 grep 更准确）✨

第3步：验证选择
检查：是否犯了常见错误？
- ❌ 用 grep 找函数定义 → 应该用 code_search（快 10-100 倍）
- ❌ 用 code_search 搜错误信息 → 应该用 grep
```

**无 CodeGraph 时**：
```
- 找代码符号？ → 查看工具列表中的符号搜索工具（如有）或用 grep
- 查调用关系？ → 查看工具列表中的调用分析工具（如有）或用 grep
```

### 2. **调试策略** - 动态化

**有 CodeGraph 时**：
```
- 定位代码：
  * 找符号定义 → code_search（优先，快 10-100 倍）✨
  * 查调用关系 → code_callers/callees（优先）✨
```

**无 CodeGraph 时**：
```
- 定位代码：
  * 找符号定义 → 使用专用符号搜索工具（如有）或 grep
  * 查调用关系 → 使用专用调用分析工具（如有）或 grep
```

### 3. **动态构建函数** - 新增

```rust
pub fn build_static_system_prompt_with_codegraph(
    profile: PromptProfile, 
    has_codegraph: bool
) -> String {
    let base_modules = profile.static_modules();
    
    // 动态选择 TOOL_DECISION 版本
    let tool_decision = if has_codegraph {
        SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH
    } else {
        SYSTEM_PROMPT_TOOL_DECISION_GENERIC
    };
    
    // 动态选择 DEBUGGING 版本
    let debugging = if has_codegraph {
        SYSTEM_PROMPT_DEBUGGING_WITH_CODEGRAPH
    } else {
        SYSTEM_PROMPT_DEBUGGING_GENERIC
    };
    
    // 在正确位置插入
    let mut modules_vec: Vec<&str> = base_modules.to_vec();
    
    // 在 TRIGGER_LOGIC 之后插入 TOOL_DECISION
    let trigger_index = modules_vec.iter()
        .position(|m| m.contains("强制触发检测"))
        .unwrap_or(4);
    modules_vec.insert(trigger_index + 1, tool_decision);
    
    // 在 TESTING 之后插入 DEBUGGING
    let testing_index = modules_vec.iter()
        .position(|m| m.contains("测试验证"))
        .unwrap_or(12);
    modules_vec.insert(testing_index + 1, debugging);
    
    modules_vec.join("\n\n")
}
```

### 4. **系统提示词构建流程优化**

```rust
// 检测是否有 CodeGraph
let has_codegraph = if let Some(path) = project_path {
    crate::tools::codegraph::should_inject_codegraph_tools(path)
} else {
    false
};

// 动态构建静态提示词（选择正确的版本）
let static_prompt = build_static_system_prompt_with_codegraph(*profile, has_codegraph);

// 添加工具列表
parts.push(tools_prompt);

// 添加 CODEGRAPH 详细规则（如有）
if has_codegraph {
    parts.push(SYSTEM_PROMPT_CODEGRAPH.to_string());
}
```

## 🎯 解决的问题

| 问题 | 之前 | 现在 |
|------|------|------|
| **优先级冲突** | 通用指引可能干扰明确指引 | 有 CodeGraph 时优先使用明确指引 |
| **Agent 困惑** | 看到通用指引但实际有更好的工具 | 明确告知使用哪个工具更高效 |
| **效率问题** | 可能使用 grep 而非 code_search | 明确说明快 10-100 倍 |
| **向后兼容** | N/A | 保持原函数签名，新增动态构建 |

## 📊 最终架构

### 有 CodeGraph 时的提示词流程

```
1. SYSTEM_PROMPT_IDENTITY
2. SYSTEM_PROMPT_SKILLS
3. SYSTEM_PROMPT_WORKFLOWS
4. SYSTEM_PROMPT_TRIGGER_LOGIC
5. SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH ✨
   └─ "找代码定义？ → code_search（优先，比 grep 快 10-100 倍）"
6. SYSTEM_PROMPT_MISSION
...
N. SYSTEM_PROMPT_DEBUGGING_WITH_CODEGRAPH ✨
   └─ "找符号定义 → code_search（优先，快 10-100 倍）"
...
工具列表
SYSTEM_PROMPT_CODEGRAPH_PRACTICE ← 实践指南
SYSTEM_PROMPT_CODEGRAPH ← 详细规则
```

### 无 CodeGraph 时的提示词流程

```
1. SYSTEM_PROMPT_IDENTITY
2. SYSTEM_PROMPT_SKILLS
...
5. SYSTEM_PROMPT_TOOL_DECISION_GENERIC ✨
   └─ "找代码符号？ → 查看工具列表中的符号搜索工具（如有）或用 grep"
...
N. SYSTEM_PROMPT_DEBUGGING_GENERIC ✨
   └─ "找符号定义 → 使用专用符号搜索工具（如有）或 grep"
```

## 🚀 优化效果

### 有 CodeGraph 时
- ✅ Agent 会优先使用 `code_search`（快 10-100 倍）
- ✅ Agent 会优先使用 `code_callers/callees`（更准确）
- ✅ 明确的常见错误纠正提示

### 无 CodeGraph 时
- ✅ Agent 知道可以使用 grep 或其他替代方案
- ✅ 不会看到不相关的 CodeGraph 指引

### 向后兼容
- ✅ 原函数 `build_static_system_prompt` 保持不变
- ✅ 新增 `build_static_system_prompt_with_codegraph` 动态构建

## ✅ 验证结果

```
✅ 所有版本常量都存在
✅ WITH_CODEGRAPH 版本包���明确指引
✅ GENERIC 版本包含通用指引
✅ 动态构建函数存在
✅ 动态构建逻辑正确
✅ 模块数组已正确移除静态引用
✅ build_system_prompt_with_workflows 使用动态构建

🎉 所有验证通过！动态构建系统正确实现。
```

## 📝 代码变更摘要

### 新增常量
- `SYSTEM_PROMPT_TOOL_DECISION_GENERIC`
- `SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH`
- `SYSTEM_PROMPT_DEBUGGING_GENERIC`
- `SYSTEM_PROMPT_DEBUGGING_WITH_CODEGRAPH`

### 新增函数
- `build_static_system_prompt_with_codegraph(profile, has_codegraph)`

### 修改函数
- `build_system_prompt_with_workflows` - 使用动态构建

### 移除
- 从 `DEFAULT_SYSTEM_PROMPT_MODULES` 移除静态 `SYSTEM_PROMPT_TOOL_DECISION`
- 从 `DEFAULT_SYSTEM_PROMPT_MODULES` 移除静态 `SYSTEM_PROMPT_DEBUGGING`
- 从 `REVIEW_SYSTEM_PROMPT_MODULES` 移除静态 `SYSTEM_PROMPT_TOOL_DECISION`

## 🎯 总结

成功实现了动态提示词构建系统，确保 Agent 在有 CodeGraph 时能够明确使用最高效的工具，在无 CodeGraph 时也能得到合适的指引。这解决了之前通用版本可能干扰 Agent 理解的问题，并提供了清晰明确的最佳实践指引。
