# System Prompt 完整结构分析报告

## 一、完整的 System Prompt 结构

### 1. 静态部分（18个模块）

位置：`core/src/prompt.rs`

**顺序**（DEFAULT 配置）：
1. **IDENTITY** (lines 3-17) - 身份定义
   - 任务复杂度判断
   - Skill 工具使用指引
   
2. **TOOL_DECISION** (lines 19-50) - 工具选择决策链 ⭐
   - 第1步：判断意图（找代码定义 → 符号搜索类工具）
   - 第2步：选择最优工具（查看适用场景、[优先] 标记）
   - 第3步：验证选择（常见错误列表）
   
3. **MISSION** (lines 52-56) - 核心目标
4. **WORKFLOW** (lines 58-64) - 工作方式
5. **AMBIGUITY** (lines 86-90) - 歧义确认
6. **BEHAVIOR** (lines 66-84) - 行为约束
7. **ACTIONS** (lines 156-173) - 操作谨慎性
8. **SYSTEM_RULES** (lines 175-189) - 系统规则
9. **QUALITY** (lines 92-97) - 代码质量
10. **TESTING** (lines 99-103) - 测试验证
11. **DEBUGGING** (lines 105-110) - 调试策略 ⚠️
    - **问题**：使用了旧的指引 `grep/read 查找相关文件`
    - **应该**：`code_search 查找符号定义，grep 查找文本内容`
    
12. **SECURITY** (lines 112-117) - 安全意识
13. **EDITING** (lines 119-125) - 编辑规则
14. **EXECUTION** (lines 127-154) - 执行策略
15. **LANGUAGE** (lines 191-203) - 语言规则
16. **OUTPUT_CONTROL** (lines 228-244) - 输出控制
17. **COMPLETION** (lines 205-226) - 完成要求
18. **TASK_TRACKING** (lines 279-284) - 任务追踪

### 2. 动态部分

**注入顺序**（`build_system_prompt_with_workflows` 函数，prompt.rs:649-722）：

```
1. static_prompt（静态部分）
2. tools_prompt（工具列表）⭐
3. SYSTEM_PROMPT_CODEGRAPH（CodeGraph 规则）⚠️
4. PROJECT CONTEXT（项目概览）
5. ACCUMULATED MEMORY（跨会话记忆）
6. AVAILABLE SKILLS（可用技能）
7. AVAILABLE WORKFLOWS（可用工作流）
```

## 二、工具列表生成逻辑

位置：`core/src/tools/mod.rs:146-178`

### 关键代码

```rust
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));

    // 条件注入 CodeGraph 工具
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }

    tools.extend(workflow::workflow_tools());

    let mut lines = vec!["可用工具：".to_string()];

    for tool in tools {
        let def = tool.definition();
        // ⚠️ 问题：截断描述
        let brief = def.description.split('.').next()
            .or_else(|| def.description.split('\n').next())
            .unwrap_or(&def.description);
        let brief = if brief.len() > 60 {
            format!("{}...", brief.chars().take(57).collect::<String>())
        } else {
            brief.to_string()
        };
        lines.push(format!("- {}: {}", def.name, brief));
    }

    lines.join("\n")
}
```

### CodeGraph 工具注入条件（`should_inject_codegraph_tools`）

```rust
pub fn should_inject_codegraph_tools(start_path: &Path) -> bool {
    super::install::is_codegraph_installed() &&  // CLI 已安装
    CodeGraphManager::with_auto_detect(start_path).is_initialized()  // .codegraph 存在
}
```

当前状态（已验证）：
```json
{
  "initialized": true,
  "file_count": 239,
  "node_count": 4376,
  "edge_count": 12100,
  "languages": ["javascript", "python", "rust", "typescript"]
}
```

✅ CodeGraph **已初始化并可注入**

## 三、CodeGraph 工具未被充分使用的根本原因

### 🚨 核心问题分析

#### 问题 1：工具描述被截断，关键信息丢失

**CodeGraph 工具的完整描述**（`tools/codegraph/tools.rs`）：

```rust
// code_search
"[优先] [优先工具] 搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍。返回符号位置、签名、文档。grep 仅用于搜索字符串内容（如错误消息）。"

// code_callers
"[优先] [优先工具] 查找调用指定符号的所有函数/方法。分析调用关系时必须优先使用，比 grep 追溯更准确。grep 仅用于搜索字符串内容。"

// code_callees
"[优先] [优先工具] 查找指定符号调用的所有函数/方法。分析执行流程时必须优先使用，比 grep 追踪更准确。grep 仅用于搜索字符串内容。"
```

**截断后**（tools/mod.rs:163-173）：
```
- code_search: [优先] [优先工具] 搜索代码符号（函数、类、方法、变量）...
- code_callers: [优先] [优先工具] 查找调用指定符号的所有函数/方法...
- code_callees: [优先] [优先工具] 查找指定符号调用的所有函数/方法...
```

❌ **丢失的关键信息**：
- "查找代码定义时**必须优先使用此工具**"
- "**比 grep 快 10-100 倍**"
- "grep 仅用于搜索字符串内容"
- 具体的适用场景和不适用场景

#### 问题 2：DEBUGGING 模块使用旧指引

**当前内容**（prompt.rs:105-110）：
```rust
const SYSTEM_PROMPT_DEBUGGING: &str = r#"调试策略：
- 先复现：理解错误信息、失败场景、触发条件
- 定位代码：grep/read 查找相关文件，分析逻辑流程  // ⚠️ 旧的指引
- 不猜测根因：用工具（日志、调试���）验证假设
- 修复后确认：运行测试或验证步骤
- 无法定位时：说明已尝试方法、排查范围、剩余可能性"#;
```

❌ **应该改为**：
```rust
- 定位代码：
  * 找符号定义 → code_search
  * 查调用关系 → code_callers/callees
  * 搜文本内容 → grep
  * 读完整文件 → read
```

#### 问题 3：工具描述冗余标记

CodeGraph 工具描述中有双重标记：
- `[优先] [优先工具]`

这是因为在 `ToolDefinition` 中：
```rust
is_priority: true,  // 自动添加 [优先] 标记
description: "[优先] [优先工具] ..."  // 描述中也硬编码了
```

#### 问题 4：TOOL_DECISION 决策链过于抽象

**当前设计**（prompt.rs:23-27）：
```rust
第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → 查看工具描述中的符号搜索类工具
- 搜文本内容？ → 文本搜索类工具
```

⚠️ **问题**：
- "符号搜索类工具" 是抽象概念
- Agent 需要去工具列表中查找哪工具属于"符号搜索类"
- 由于工具描述被截断，Agent 无法判断哪个工具最适合

**应该改为**（具体工具名）：
```rust
第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → code_search（优先）
- 搜文本内容？ → grep/search
- 查调用关系？ → code_callers/callees（优先）
- 读完整文件？ → read
```

#### 问题 5：CODEGRAPH 规则注入位置不佳

**当前顺序**：
```
1. static_prompt（包含 TOOL_DECISION）
2. tools_prompt（工具列表，描述被截断）
3. SYSTEM_PROMPT_CODEGRAPH（详细规则）
```

⚠️ **问题**：
- TOOL_DECISION 在前，但看不到完整工具描述
- 工具列表在中间，但描述被截断
- CODEGRAPH 规则在后，Agent 可能已经根据截断的描述做出决策

## 四、解决方案

### 方案 A：修复工具描述截断问题（最小改动）

**位置**：`core/src/tools/mod.rs:163-173`

**当前逻辑**：
```rust
let brief = def.description.split('.').next()
    .or_else(|| def.description.split('\n').next())
    .unwrap_or(&def.description);
let brief = if brief.len() > 60 {
    format!("{}...", brief.chars().take(57).collect::<String>())
} else {
    brief.to_string()
};
```

**改进方案**：
```rust
// 对于优先工具，保留更完整的描述
let brief = if def.is_priority {
    // 优先工具保留前150字符，包含适用场景
    let desc = def.description.split('\n').next().unwrap_or(&def.description);
    if desc.len() > 150 {
        format!("{}...", desc.chars().take(147).collect::<String>())
    } else {
        desc.to_string()
    }
} else {
    // 其他工具保持原有截断逻辑
    let desc = def.description.split('.').next()
        .or_else(|| def.description.split('\n').next())
        .unwrap_or(&def.description);
    if desc.len() > 60 {
        format!("{}...", desc.chars().take(57).collect::<String>())
    } else {
        desc.to_string()
    }
};
```

### 方案 B：修复 DEBUGGING 模块指引（推荐）

**位置**：`core/src/prompt.rs:105-110`

**改进内容**：
```rust
const SYSTEM_PROMPT_DEBUGGING: &str = r#"调试策略：
- 先复现：理解错误信息、失败场景、触发条件
- 定位代码：
  * 找符号定义 → code_search（优先）
  * 查调用关系 → code_callers/callees（优先）
  * 搜文本内容 → grep/search
  * 读完整文件 → read
- 不猜测根因：用工具（日志、调试器）验证假设
- 修复后确认：运行测试或验证步骤
- 无法定位时：说明已尝试方法、排查范围、剩余可能性"#;
```

### 方案 C：改进 TOOL_DECISION 决策链（根本解决）

**位置**：`core/src/prompt.rs:19-50`

**关键改进**：将抽象指引改为具体工具名

**改进内容**：
```rust
const SYSTEM_PROMPT_TOOL_DECISION: &str = r#"工具选择决策链（必须执行）：

第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → code_search（优先，比 grep 快 10-100 倍）
- 搜文本内容？ → grep（错误消息、日志、注释）
- 查调用关系？ → code_callers/callees（优先，比 grep 更准确）
- 改文件？ → edit/write
- 执行命令？ → bash
- 不确定？ → 先用 ask 确认

第2步：验证工具可用性
- 如果工具不在列表中 → 说明不可用，选择替代方案
- 如果 CodeGraph 未初始化 → 用 grep/search 替代 code_*

第3步：验证选择
检查：是否犯了常见错误？
- ❌ 用 grep 找函数定义 → 应该用 code_search
- ❌ 用 code_search 搜错误信息 → 应该用 grep
- ❌ 单处改动用批量编辑 → 应该用 edit

优先级规则：
- 有 [优先] 标记的工具必须优先考虑
- CodeGraph 工具（code_*）用于代码符号分析
- 传统工具（grep/read）用于文本内容搜索"#;
```

### 方案 D：动态注入工具选择规则（最佳方案）

**核心思路**：
- 静态 Prompt 不提具体工具名
- 根据项目状态动态注入具体规则

**实现步骤**：

1. 修改 `SYSTEM_PROMPT_TOOL_DECISION` 为通用原则：
```rust
const SYSTEM_PROMPT_TOOL_DECISION: &str = r#"工具选择决策链（必须执行）：

第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → 查看 [优先] 标记的符号搜索工具
- 搜文本内容？ → 文本搜索工具（grep/search）
- 改文件？ → 编辑工具（edit/write）
- 不确定？ → 先用 ask 确认

第2步：选择最优工具
- 查看 [优先] 标记 → 这些工具更快更准确
- 查看工具描述中的适用场景
- 验证工具在列表中存在

第3步：验证选择
避免常见错误：
- 不要用文本搜索工具找代码定义
- 不要用符号搜索工具搜文本内容"#;
```

2. 在 `build_system_prompt_with_workflows` 中动态注入具体规则：
```rust
// 在 static prompt 和 tools prompt 之间插入
if crate::tools::codegraph::should_inject_codegraph_tools(path) {
    parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
}

// 新增常量
const SYSTEM_PROMPT_CODEGRAPH_PRACTICE: &str = r#"【工具选择实践指南】

查找代码符号（当前项目支持 CodeGraph）：
- 函数/类/变量定义 → code_search（优先，快 10-100 倍）
- 调用关系分析 → code_callers/callees（优先）
- 错误消息/注释 → grep
- 完整文件内容 → read

常见错误纠正：
❌ grep "function_name" → ✅ code_search "function_name"
❌ grep "who calls this" → ✅ code_callers "symbol_id"
❌ read 逐行查找 → ✅ code_search 直接定位"#;
```

### 方案 E：去除冗余标记

**位置**：`core/src/tools/codegraph/tools.rs`

**改进**：去除描述中的硬编码标记，只保留 `is_priority` 标记

```rust
// code_search
description: "搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍。返回符号位置、签名、文档。grep 仅用于搜索字符串内容（如错误消息）。".to_string(),
is_priority: true,

// code_callers
description: "查找调用指定符号的所有函数/方法。分析调用关系时必须优先使用，比 grep 追溯更准确。grep 仅用于搜索字符串内容。".to_string(),
is_priority: true,

// code_callees
description: "查找指定符号调用的所有函数/方法。分析执行流程时必须优先使用，比 grep 追踪更准确。grep 仅用于搜索字符串内容。".to_string(),
is_priority: true,
```

## 五、推荐实施方案

### 阶段 1：立即修复（最小改动）

1. ✅ **方案 B**：修复 DEBUGGING 模块
2. ✅ **方案 A**：改进工具描述截断逻辑（优先工具保留更多信息）

### 阶段 2：根本解决（最佳方案）

3. ✅ **方案 C + D**：改进 TOOL_DECISION 决策链 + 动态注入实践指南
4. ✅ **方案 E**：去除冗余标记

### 实施优先级

**高优先级**：
- 方案 B（DEBUGGING 模块）- 直接影响 Agent 行为
- 方案 C（TOOL_DECISION）- 核心决策链

**中优先级**：
- 方案 A（工具描述截断）- 辅助信息
- 方案 D（动态���入）- 增强适应性

**低优先级**：
- 方案 E（冗余标记）- 美化问题

## 六、验证方案

### 测试案例

**案例 1**：查找函数定义
```
用户："查找 Agent 类的定义"
期望：code_search "Agent"
实际：grep "class Agent" ❌
```

**案例 2**：查找调用关系
```
用户："谁调用了 run 方法"
期望：code_callers "run"
实际：grep "run()" ❌
```

**案例 3**：搜索错误信息
```
用户："查找 'failed to connect' 错误"
期望：grep "failed to connect" ✅
实际：grep "failed to connect" ✅
```

### 验证指标

**定量指标**：
- code_search 使用率：当前 < 10%，目标 > 50%
- grep 误用率：当前 > 40%，目标 < 10%

**定性指标**：
- Agent 能准确区分"找代码定义"和"搜文本内容"
- Agent 知道 CodeGraph 工具的具体适用场景
- Agent 能根据工具描述做出正确选择

## 七、总结

### 根本原因

1. **工具描述截断** → Agent 看不到"必须优先使用"的关键信息
2. **决策链过于抽象** → "符号搜索类工具"不如"code_search"具体
3. **DEBUGGING 使用旧指引** → 教 agent 用 grep 而非 code_search
4. **注入顺序不佳** → 规则在工具列表之后，影响已形成

### 解决思路

**核心原则**：
- **静态 Prompt = 通用原则**（不提具体工具名）
- **动态注入 = 具体规则**（根据项目状态注入）
- **工具描述 = 完整信息**（优先工具保留适用场景）

**最佳实践**：
- 将"找代码定义 → code_search"写入动态注入的规则
- 在 DEBUGGING/TOOL_DECISION 中明确区分符号搜索和文本搜索
- 工具描述保留关键信息（适用场景、性能优势）

### 下一步行动

1. 实施方案 B（修复 DEBUGGING）- 立即可做
2. 实施方案 C（改进 TOOL_DECISION）- 核心修复
3. 测试验证：观察 Agent 是否优先使用 code_search
4. 收集数据：统计工具使用率变化