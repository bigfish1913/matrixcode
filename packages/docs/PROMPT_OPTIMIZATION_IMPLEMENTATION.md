# MatrixCode 提示词优化实施报告

## 实施日期
2025-05-28

## 🚨 发现的严重问题

### 问题：静态提示词中硬编码可能不存在的工具

**问题描述**：
静态提示词中硬编码了 17 处 CodeGraph 工具引用（`code_search`、`code_callers`、`code_callees`），但这些工具**可能不可用**（需要 CodeGraph CLI 安装 + `.codegraph` 目录初始化）。

**影响**：
- Agent 在没有 CodeGraph 的项目中会尝试使用不存在的工具
- 导致工具调用失败，影响用户体验
- 违反了"静态提示词不应该引用可能不存在的工具"原则

**修复方案**：
- 静态提示词只包含**通用原则**（不提具体工具名）
- 动态提示词包含**具体规则**（通过 `SYSTEM_PROMPT_CODEGRAPH_PRACTICE` 动态注入）

---

## 实施内容

### ✅ 高优先级优化（已完成）

#### 1. 工具描述截断优化

**位置**: `core/src/tools/mod.rs:146-195`

**改动内容**:
- 分类显示工具：优先工具 + 其他工具
- 优先工具保留完整描述（最多150字符）
- 其他工具保持简要描述（最多60字符）
- 使用 `description_for_llm()` 自动添加 `[优先]` 标记

**优化前**:
```
可用工具：
- code_search: [优先] 搜索代码符号（函数、类、方法、变量）...
- read: 读取指定路径的文件内容
```

**优化后**:
```
可用工具：

【优先工具 - 必须优先考虑】
  code_search: [优先] 搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍。返回符号位置、签名、文档。grep 仅用于搜索字符串内容（如错误消息）。
  code_callers: [优先] 查找调用指定符号的所有函数/方法。分析调用关系时必须优先使用，比 grep 追溯更准确。grep 仅用于搜索字符串内容。
  code_callees: [优先] 查找指定符号调用的所有函数/方法。分析执行流程时必须优先使用，比 grep 追踪更准确。grep 仅用于搜索字符串内容。

【其他工具】
  read: 读取指定路径的文件内容
  write: 向文件写入内容，若文件不存在则创建...
  edit: 在文件中查找精确匹配的字符串并替换为新内容...
```

**效果**:
- Agent 可以看到完整的关键信息："比 grep 快 10-100 倍"、"必须优先使用"
- 优先工具有独立的分类标题，Agent 明确知道哪些工具必须优先考虑

---

#### 2. 提示词注入顺序优化

**位置**: `core/src/prompt.rs:828-838`

**改动内容**:
调整提示词注入顺序，让实践指南在工具列表之前：

**优化前的顺序**:
```
1. static_prompt (包含 TOOL_DECISION)
2. tools_prompt (工具列表)
3. SYSTEM_PROMPT_CODEGRAPH_PRACTICE
4. SYSTEM_PROMPT_CODEGRAPH
```

**优化后的顺序**:
```
1. static_prompt (包含 TOOL_DECISION)
2. SYSTEM_PROMPT_CODEGRAPH_PRACTICE ← 新位置
3. tools_prompt (工具列表)
4. SYSTEM_PROMPT_CODEGRAPH
```

**效果**:
- Agent 在看到工具列表之前就知道如何选择工具
- 工具选择决策链有具体的工具名作为参考
- 避免 Agent 根据截断的描述做出错误决策

---

#### 3. 冗余标记去除

**位置**: `core/src/tools/codegraph/tools.rs`

**改动内容**:
去除 CodeGraph 工具描述中的硬编码 `[优先]` 标记，只保留 `is_priority: true`

**优化前**:
```rust
description: "[优先] 搜索代码符号（函数、类、方法、变量）...".to_string(),
is_priority: true,  // 自动添加 [优先] 标记
```

**优化后**:
```rust
description: "搜索代码符号（函数、类、方法、变量）...".to_string(),
is_priority: true,  // 通过 description_for_llm() 自动添加 [优先] 标记
```

**效果**:
- 避免双重标记：`[优先] [优先]`
- 减少 token 浪费
- 标记管理更统一（通过 `is_priority` 字段）

---

#### 4. 静态提示词去工具名化（关键修复）🔧

**位置**: `core/src/prompt.rs`

**改动内容**:
从静态提示词中移除所有硬编码的工具名引用，改为通用原则。

**修改 1: SYSTEM_PROMPT_TOOL_DECISION**

优化前：
```
第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → code_search（优先，比 grep 快 10-100 倍）
- 搜文本内容？ → grep（错误消息、日志、注释）
- 查调用关系？ → code_callers/callees（优先，比 grep 更准确）
...

第2步：验证工具可用性
- 如果工具不在列表中 → 说明不可用，选择替代方案
- 如果 CodeGraph 未初始化 → 用 grep/search 替代 code_*
```

优化后：
```
第1步：判断意图
问自己：用户想做什么？
- 找代码符号？ → 查看工具列表中的符号搜索工具（如有）
- 搜文本内容？ → grep（错误消息、日志、注释）
- 查调用关系？ → 查看工具列表中的调用分析工具（如有）
...

第2步：验证工具可用性
- 检查工具是否在可用工具列表中
- 如果工具不在列表中 → 说明不可用，选择替代方案
- 优先使用带有 [优先] 标记的工具
```

**修改 2: SYSTEM_PROMPT_DEBUGGING**

优化前：
```
定位代码：
  * 找符号定义 → code_search（优先）
  * 查调用关系 → code_callers/callees（优先）
```

优化后：
```
定位代码：
  * 找符号定义 → 使用专用符号搜索工具（如有）或 grep
  * 查调用关系 → 使用专用调用分析工具（如有）或 grep
```

**修改 3: SYSTEM_PROMPT_SKILLS 示例**

优化前：
```
→ 立即执行指令，调用 code_search 查找用户输入处理代码
```

优化后：
```
→ 立即执行指令，使用符号搜索工具查找用户输入处理代码
```

**修改 4: SYSTEM_PROMPT_MEMORY**

优化前：
```
- 如果记忆命名了函数：用 code_search 搜索验证
```

优化后：
```
- 如果记忆命名了函数：先用符号搜索工具验证（如有）或用 grep 搜索
```

**原理说明**：
- ✅ `SYSTEM_PROMPT_CODEGRAPH_PRACTICE` 和 `SYSTEM_PROMPT_CODEGRAPH` 可以包含工具名
  - 因为它们是**动态注入**的，只在 CodeGraph 工具可用时注入
- ❌ `SYSTEM_PROMPT_TOOL_DECISION` 等静态提示词不能包含工具名
  - 因为它们**始终存在**，即使工具不可用

---

### ✅ 测试验证

#### 编译测试
```bash
cd packages/core && cargo build
```
**结果**: ✅ 编译成功（2.20秒）

#### 单元测试
```bash
cd packages/core && cargo test --lib
```
**结果**: ✅ 226/227 测试通过
- test_all_tools_includes_workflow_tools ... ok
- test_generate_tools_prompt_without_path_excludes_codegraph ... ok
- test_generate_tools_prompt_includes_workflow ... ok
- test_generate_tools_prompt_with_path_includes_codegraph ... ok
- 其他 222 个测试 ... ok

**失败测试**（与修改无关）：
- mcp::types::tests::test_tool_deserialization（已存在的失败）

---

## 预期效果

### 定量指标

**优化前**（基于 SYSTEM_PROMPT_ANALYSIS.md）:
- code_search 使用率：< 10%
- grep 误用率：> 40%

**优化后目标**:
- code_search 使用率：> 50%（提升 5 倍）
- grep 误用率：< 10%（降低 4 倍）

---

### 定性指标

#### Agent 行为预期改进

**优化前的问题**:
1. Agent 经常先用 grep 查找函数定义
2. Agent 不知道 code_search 比 grep 快
3. Agent 忽略 [优先] 标记
4. 工具描述截断导致关键信息丢失
5. **静态提示词引用不存在的工具**

**优化后的预期**:
1. Agent 优先使用 code_search 查找符号（如果有）
2. Agent 明确知道性能优势（"快 10-100 倍"）
3. Agent 遵循优先工具分类指引
4. Agent 能看到完整的适用场景说明
5. **静态提示词只包含通用原则，动态注入具体规则**

---

## 技术细节

### 工具分类逻辑

```rust
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));
    
    // 条件注入 CodeGraph 工具
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }
    
    tools.extend(workflow::workflow_tools());
    
    // 🎯 分类显示
    let mut priority_tools = Vec::new();
    let mut normal_tools = Vec::new();
    
    for tool in tools {
        let def = tool.definition();
        if def.is_priority {
            priority_tools.push(def);
        } else {
            normal_tools.push(def);
        }
    }
    
    // 优先工具：完整描述（150字符）
    // 其他工具：简要描述（60字符）
    // ...
}
```

---

### 提示词构建流程

```rust
pub fn build_system_prompt_with_workflows(...) -> String {
    let static_prompt = build_static_system_prompt(*profile);
    let tools_prompt = crate::tools::generate_tools_prompt_with_path(project_path);
    
    // 🎯 实践指南在工具列表之前（仅当 CodeGraph 可用时注入）
    let mut parts = vec![static_prompt];
    
    if let Some(path) = project_path
        && crate::tools::codegraph::should_inject_codegraph_tools(path) {
        parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
    }
    
    parts.push(tools_prompt);
    
    if let Some(path) = project_path
        && crate::tools::codegraph::should_inject_codegraph_tools(path) {
        parts.push(SYSTEM_PROMPT_CODEGRAPH.to_string());
    }
    
    // ...
}
```

### 动态注入条件

```rust
pub fn should_inject_codegraph_tools(start_path: &Path) -> bool {
    // 必须同时满足两个条件：
    // 1. CodeGraph CLI 已安装
    super::install::is_codegraph_installed() && 
        // 2. 项目已初始化（存在 .codegraph 目录）
        CodeGraphManager::with_auto_detect(start_path).is_initialized()
}
```

---

## 与 Claude Code 对比

### Claude Code 的做法

1. **分散的工具指引**: 每个工具描述说明适用场景，但无统一决策链
2. **详细的 Git 指引**: 包含具体的 commit/PR 创建步骤
3. **丰富的示例**: 每个工具都有具体的命令示例

### MatrixCode 的做法（优化后）

1. **统一的决策链**: TOOL_DECISION + CODEGRAPH_PRACTICE 双重指引
2. **分类显示**: 优先工具独立分类 + 完整描述
3. **动态注入**: 根据项目状态注入具体规则
4. **静态通用原则**: 不依赖可能不存在的工具

### 优势对比

| 维度 | Claude Code | MatrixCode (优化后) |
|------|-------------|---------------------|
| 工具选择指引 | 分散在各工具描述 | ✅ 统一决策链 + 分类显示 |
| 优先级标记 | 无 | ✅ [优先] 标记 + 独立分类 |
| 性能数据 | 无 | ✅ "快 10-100 倍" |
| 适用场景 | 在工具描述中 | ✅ 在实践指南 + 工具描述 |
| 错误纠正 | 无 | ✅ 常见错误纠正示例 |
| 动态注入 | 无 | ✅ 根据项目状态注入 |
| 静态提示词 | 不涉及工具 | ✅ 通用原则，不依赖工具 |

---

## 后续工作

### 中优先级（待实施）

#### 1. Skill/Workflow 自动触发检测

**目标**: 提高触发准确性

**方案**:
- 自动检测用户意图关键词
- 在系统提示词中添加触发规则
- 自动注入触发检测提示

**预期工作量**: 中（需要设计检测逻辑）

---

#### 2. Git 操作详细指引

**目标**: 提高 Git 操作质量

**方案**: 借鉴 Claude Code 的详细 Git 指引
- 7 个步骤的 commit 创建流程
- 3 个步骤的 PR 创建流程
- 具体的命令示例

**预期工作量**: 小

---

### 低优先级（可选）

#### 3. 工具描述进一步优化

**方案**: 添加更多示例和适用场景

**预期工作量**: 小

---

## 总结

### 实施成果

1. ✅ **工具描述截断优化**: 分类显示 + 优先工具完整描述
2. ✅ **提示词注入顺序优化**: 实践指南在工具列表之前
3. ✅ **冗余标记去除**: 避免双重 [优先] 标记
4. ✅ **静态提示词去工具名化**: 修复严重问题
5. ✅ **测试验证**: 226/227 测试通过

### 核心改进

- **Agent 能看到关键信息**: "比 grep 快 10-100 倍"、"必须优先使用"
- **Agent 有明确的指引**: 分类显示 + 实践指南 + 错误纠正
- **减少 token 浪费**: 去除冗余标记
- **修复严重问题**: 静态提示词不再引用可能不存在的工具

### 预期效果

- **code_search 使用率**: 从 <10% 提升到 >50%（提升 5 倍）
- **grep 误用率**: 从 >40% 降低到 <10%（降低 4 倍）
- **工具可用性问题**: 完全修复静态提示词引用不存在工具的问题

---

## 验证建议

### 测试案例

**案例 1**: 查找函数定义（有 CodeGraph）
```
用户："查找 Agent 类的定义"
期望：code_search "Agent"
观察：工具调用序列
```

**案例 2**: 查找调用关系（有 CodeGraph）
```
用户："谁调用了 run 方法"
期望：code_callers "run"
观察：工具调用序列
```

**案例 3**: 搜索错误信息（有 CodeGraph）
```
用户："查找 'failed to connect' 错误"
期望：grep "failed to connect"
观察：工具调用序列
```

**案例 4**: 无 CodeGraph 项目
```
用户："查找 Agent 类的定义"
期望：grep 或 search（不应尝试调用 code_search）
观察：工具调用序列（检查是否遵循静态提示词的通用原则）
```

---

## 附录：改动文件列表

1. `core/src/tools/mod.rs` - 工具描述截断优化
2. `core/src/prompt.rs` - 提示词注入顺序优化 + 静态提示词去工具名化
3. `core/src/tools/codegraph/tools.rs` - 冗余标记去除
4. `core/src/config.rs` - 测试配置修复

---

## 参考文档

1. `PROMPT_OPTIMIZATION_ANALYSIS.md` - 对比分析报告
2. `SYSTEM_PROMPT_ANALYSIS.md` - 系统提示词分析
3. Claude Code 源码: `claude-code-analysis/src/tools/*/prompt.ts`