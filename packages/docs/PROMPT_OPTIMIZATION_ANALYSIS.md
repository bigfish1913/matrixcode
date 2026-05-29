# Claude Code vs MatrixCode 提示词对比分析与优化建议

## 一、整体架构对比

### 1.1 Claude Code 提示词架构

**特点**：
- **工具提示词分离**：每个工具有独立的 `prompt.ts` 文件
- **动态生成**：通过函数动态渲染提示词（`renderPromptTemplate`）
- **上下文感知**：根据运行时环境动态注入不同内容
- **模块化**：每个工具的提示词独立维护，易于扩展

**示例**（FileReadTool）：
```typescript
export function renderPromptTemplate(
  lineFormat: string,
  maxSizeInstruction: string,
  offsetInstruction: string,
): string {
  return `Reads a file from the local filesystem...`
}
```

### 1.2 MatrixCode 提示词架构

**特点**：
- **静态常量**：使用 `const` 定义静态提示词模块
- **统一管理**：所有提示词在 `prompt.rs` 中集中管理
- **动态注入**：根据项目状态动态注入 CodeGraph 规则
- **优先级标记**：工具定义中包含 `is_priority` 字段

**示例**（prompt.rs）：
```rust
const SYSTEM_PROMPT_TOOL_DECISION: &str = r#"工具选择决策链（必须执行）：
第1步：判断意图
问自己：用户想做什么？
- 找代码定义？ → code_search（优先，比 grep 快 10-100 倍）
..."#;
```

---

## 二、关键差异分析

### 2.1 工具选择决策

#### Claude Code 的做法
- **分散在各工具描述中**：每个工具描述说明适用场景
- **无统一决策链**：依赖 Agent 自己判断
- **示例**：
  ```
  IMPORTANT: 当有相关专用工具时，不要用此工具运行命令。
  使用专用工具更好：
  | 命令 | 替代工具 |
  |-----|---------|
  | cat/head/tail | read |
  | sed/awk | edit |
  ```

#### MatrixCode 的做法
- **统一的决策链**：在 `SYSTEM_PROMPT_TOOL_DECISION` 中明确定义
- **三步验证流程**：
  1. 判断意图
  2. 验证工具可用性
  3. 验证选择
- **明确的优先级**：
  ```
  - 找代码定义？ → code_search（优先，比 grep 快 10-100 倍）
  - 搜文本内容？ → grep（错误消息、日志、注释）
  ```

**优势对比**：
- ✅ MatrixCode 更明确：告诉 Agent 具体用什么工具
- ✅ MatrixCode 有优先级：`[优先]` 标记 + 性能数据
- ✅ MatrixCode 有纠错：常见错误示例

---

### 2.2 工具描述详略

#### Claude Code
- **详细说明适用场景**
- **包含反例**：`grep 仅用于搜索字符串内容（如错误消息）`
- **示例**（BashTool）：
  ```
  IMPORTANT: Avoid using find, grep, cat, head, tail, sed, awk, or echo commands.
  Instead, use the appropriate dedicated tool...
  ```

#### MatrixCode
- **被截断**（tools/mod.rs:163-173）：
  ```rust
  let brief = def.description.split('.').next()
      .or_else(|| def.description.split('\n').next())
      .unwrap_or(&def.description);
  if brief.len() > 60 {
      format!("{}...", brief.chars().take(57).collect::<String>())
  }
  ```
- **结果**：
  - `code_search: [优先] 搜索代码符号（函数、类、方法、变量）...`
  - ❌ 丢失了关键信息："比 grep 快 10-100 倍"、"必须优先使用"

**问题**：MatrixCode 的工具描述截断导致 Agent 看不到关键信息。

---

### 2.3 调试策略指引

#### Claude Code
- **无专门的调试策略模块**
- 调试指引分散在各工具描述中

#### MatrixCode
- **专门的 DEBUGGING 模块**：
  ```rust
  const SYSTEM_PROMPT_DEBUGGING: &str = r#"调试策略：
  - 先复现：理解错误信息、失败场景、触发条件
  - 定位代码：
    * 找符号定义 → code_search（优先）
    * 查调用关系 → code_callers/callees（优先）
    * 搜文本内容 → grep/search
    * 读完整文件 → read
  ...
  "#;
  ```

**优势**：MatrixCode 的调试策略更系统、更有指导性。

---

### 2.4 Git 安全协议

#### Claude Code
- **详细的 Git 操作指引**（BashTool/prompt.ts:81-161）
- 包含：
  - Commit 创建步骤（7 个步骤）
  - PR 创建步骤（3 个步骤）
  - Git 安全规则（5 条）
  - 常见操作示例

#### MatrixCode
- **简洁的安全规则**：
  ```rust
  const SYSTEM_PROMPT_GIT_SAFETY: &str = r#"【Git Safety Protocol】
  只在用户要求时创建 commit。不清楚就先问。
  
  安全规则：
  - 绝不要更新 git config
  - 绝不要运行破坏性命令（push --force、reset --hard...）
  - 绝不要跳过 hooks（--no-verify...）
  - CRITICAL: 总是创建新 commit 而非 amend
  "#;
  ```

**对比**：
- Claude Code 更详细：适合新手
- MatrixCode 更简洁：适合有经验的用户

---

### 2.5 Skill/Workflow 系统

#### Claude Code
- **Skill 系统**：通过 `/command` 触发
- **Workflow 系统**：无

#### MatrixCode
- **Skill 系统**：有详细的触发检测机制
  ```rust
  const SYSTEM_PROMPT_SKILLS: &str = r#"【Skills 技能系统 - 核心特性】
  🔴 **重要程度**: 最高优先级 - 遇到匹配场景必须优先调用
  
  【触发机制 - 自动识别】
  - 用户说 "/review" 或 "审查代码" → 调用 "code-review" skill
  ...
  "#;
  ```
- **Workflow 系统**：有完整的自动化指引
  ```rust
  const SYSTEM_PROMPT_WORKFLOWS: &str = r#"【Workflows 工作���系统 - 核心特性】
  - 用户请求包含多个步骤 → 必须先考虑 Workflow
  - 用户请求研究型任务 → 必须先考虑 Workflow
  ...
  "#;
  ```

**优势**：MatrixCode 的 Skill/Workflow 系统更系统化、有强制触发机制。

---

### 2.6 风险管理

#### Claude Code
- 无专门的风险管理模块

#### MatrixCode
- **三级风险分类**：
  ```rust
  const SYSTEM_PROMPT_RISK_MANAGEMENT: &str = r#"【操作风险分级】
  🟢 **低风险 - 自由执行**
  - 本地、可逆操作：编辑文件、运行测试
  
  🟡 **中风险 - 提醒用户**
  - 影响范围可控：修改多个文件
  
  🔴 **高风险 - 必须强制确认**
  - 破坏性：删除文件/目录/分支
  - 难逆转：force-push、git reset --hard
  ...
  "#;
  ```

**优势**：MatrixCode 的风险管理更清晰、更有层次。

---

## 三、MatrixCode 存在的问题

### 3.1 工具描述截断问题（已分析文档中提到）

**位置**：`core/src/tools/mod.rs:163-173`

**问题**：
```rust
let brief = def.description.split('.').next()
    .or_else(|| def.description.split('\n').next())
    .unwrap_or(&def.description);
if brief.len() > 60 {
    format!("{}...", brief.chars().take(57).collect::<String>())
}
```

**影响**：
- CodeGraph 工具的关键信息被截断
- Agent 看不到"比 grep 快 10-100 倍"
- Agent 不知道"必须优先使用"

**解决方案**（已在分析文档中提出）：
```rust
// 对于优先工具，保留更完整的描述
let brief = if def.is_priority {
    let desc = def.description.split('\n').next().unwrap_or(&def.description);
    if desc.len() > 150 {
        format!("{}...", desc.chars().take(147).collect::<String>())
    } else {
        desc.to_string()
    }
} else {
    // 其他工具保持原有截断逻辑
    ...
};
```

---

### 3.2 提示词注入顺序问题（已在分析文档中提到）

**当前顺序**：
```
1. static_prompt（包含 TOOL_DECISION）
2. tools_prompt（工具列表，描述被截断）
3. SYSTEM_PROMPT_CODEGRAPH（详细规则）
```

**问题**：
- TOOL_DECISION 在前，但看不到完整工具描述
- 工具列表在中间，但描述被截断
- CODEGRAPH 规则在后，Agent 可能已经根据截断的描述做出决策

**解决方案**（已在分析文档中提出）：
- 调整注入顺序，让 CODEGRAPH 规则在工具列表之前
- 或者在 TOOL_DECISION 中明确提到具体工具名

---

### 3.3 冗余标记问题（已在分析文档中提到）

**问题**：
```rust
// tools/codegraph/tools.rs
description: "[优先] [优先工具] 搜索代码符号...".to_string(),
is_priority: true,  // 自动添加 [优先] 标记
```

**结果**：
- Agent 看到双重标记：`[优先] [优先工具]`
- 浪费 token

**解决方案**：
- 去除描述中的硬编码标记
- 只保留 `is_priority: true`

---

## 四、优化建议

### 4.1 工具描述优化（高优先级）

#### 方案 A：改进截断逻辑

```rust
// core/src/tools/mod.rs
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));
    
    // ... 工具收集逻辑 ...
    
    let mut lines = vec!["可用工具：".to_string()];
    
    for tool in tools {
        let def = tool.definition();
        
        // 🎯 关键改进：优先工具保留更完整的描述
        let brief = if def.is_priority {
            // 优先工具保留前 150 字符，包含适用场景
            let desc = def.description.split('\n').next().unwrap_or(&def.description);
            if desc.len() > 150 {
                format!("{}...", desc.chars().take(147).collect::<String>())
            } else {
                desc.to_string()
            }
        } else {
            // 其他工具保持原有截断逻辑（60 字符）
            let desc = def.description.split('.').next()
                .or_else(|| def.description.split('\n').next())
                .unwrap_or(&def.description);
            if desc.len() > 60 {
                format!("{}...", desc.chars().take(57).collect::<String>())
            } else {
                desc.to_string()
            }
        };
        
        lines.push(format!("- {}: {}", def.name, brief));
    }
    
    lines.join("\n")
}
```

**效果**：
- 优先工具描述从 60 字符增加到 150 字符
- Agent 可以看到"比 grep 快 10-100 倍"等关键信息

---

#### 方案 B：分类显示工具

```rust
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));
    
    // 条件注入 CodeGraph 工具
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }
    
    tools.extend(workflow::workflow_tools());
    
    // 🎯 关键改进：分类显示
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
    
    let mut lines = vec!["可用工具：".to_string()];
    
    // 优先工具（完整描述）
    if !priority_tools.is_empty() {
        lines.push("\n【优先工具 - 必须优先考虑】".to_string());
        for def in priority_tools {
            lines.push(format!("  - {}: {}", def.name, def.description));
        }
    }
    
    // 其他工具（简要描述）
    if !normal_tools.is_empty() {
        lines.push("\n【其他工具】".to_string());
        for def in normal_tools {
            let brief = def.description.split('.').next()
                .or_else(|| def.description.split('\n').next())
                .unwrap_or(&def.description);
            let brief = if brief.len() > 60 {
                format!("{}...", brief.chars().take(57).collect::<String>())
            } else {
                brief.to_string()
            };
            lines.push(format!("  - {}: {}", def.name, brief));
        }
    }
    
    lines.join("\n")
}
```

**效果**：
- 优先工具有独立的分类标题
- Agent 明确知道哪些工具必须优先考虑
- 优先工具显示完整描述

---

### 4.2 提示词结构优化（高优先级）

#### 方案：调整注入顺序

```rust
// core/src/prompt.rs
pub fn build_system_prompt_with_workflows(
    // ...
) -> String {
    let mut parts = vec![];
    
    // 1. 静态部分
    parts.push(static_prompt);
    
    // 🎯 关键改进：CodeGraph 规则在工具列表之前
    if let Some(path) = project_path
        && crate::tools::codegraph::should_inject_codegraph_tools(path) {
        parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
    }
    
    // 2. 工具列表（此时 Agent 已经知道 CodeGraph 规则）
    parts.push(tools_prompt);
    
    // 3. 项目上下文
    if let Some(context) = project_context {
        parts.push(context);
    }
    
    // 4. 其他部分
    // ...
    
    parts.join("\n\n")
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

**效果**：
- Agent 在看到工具列表之前就知道如何选择工具
- 工具选择决策链有具体的工具名作为参考

---

### 4.3 调试策略优化（已实施）

**当前状态**：已在 `SYSTEM_PROMPT_DEBUGGING` 中优化

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

**验证**：✅ 已正确引导 Agent 使用 CodeGraph 工具

---

### 4.4 工具选择决策链优化（已实施）

**当前状态**：已在 `SYSTEM_PROMPT_TOOL_DECISION` 中优化

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

并行调用规则：
- 多个独立工具调用可在单次响应中并行发出
- 依赖其他调用结果的工具必须顺序调用
- 最大化并行以提高效率

优先级规则：
- 有 [优先] 标记的工具必须优先考虑
- 根据工具描述中的"适用场景"选择合适工具"#;
```

**验证**：✅ 已明确提到具体工具名 + 性能数据

---

### 4.5 Skill/Workflow 触发优化（中优先级）

#### 问题
当前触发机制依赖 Agent 自主检测，可能遗漏。

#### 优化方案：自动注入触发检测

```rust
// 在每次用户输入后，系统自动检测
pub fn detect_skill_or_workflow(user_input: &str) -> Option<String> {
    let input_lower = user_input.to_lowercase();
    
    // Skill 触发检测
    if input_lower.contains("审查") || input_lower.contains("review") {
        return Some("skill:code-review");
    }
    if input_lower.contains("重构") || input_lower.contains("refactor") {
        return Some("skill:refactor");
    }
    
    // Workflow 触发检测
    if input_lower.contains("生成") && input_lower.contains("报告") {
        return Some("workflow:generate-report");
    }
    
    None
}

// 在系统提示词中添加
const SYSTEM_PROMPT_AUTO_TRIGGER: &str = r#"【自动触发检测】

系统会在以下场景自动触发 Skill/Workflow：

Skills:
- 用户说 "审查代码" → 自动调用 code-review skill
- 用户说 "重构代码" → 自动调用 refactor skill

Workflows:
- 用户说 "生成报告" → 自动调用 generate-report workflow
- 用户说 "批量处理" → 自动调用 batch-process workflow

如果检测到触发，系统会自动调用对应工具，无需您手动指定。"#;
```

---

### 4.6 Git 操作优化（低优先级）

#### 对比 Claude Code 的详细指引

Claude Code 的 Git 提交指引非常详细：
- 7 个步骤
- 每个步骤都有具体命令
- 包含示例

#### MatrixCode 可以借鉴的部分

```rust
const SYSTEM_PROMPT_GIT_COMMIT_GUIDE: &str = r#"【Git 提交指南】

当用户要求创建 commit 时：

1. 查看状态（并行执行）：
   - git status
   - git diff
   - git log --oneline -5

2. 分析变更：
   - 理解变更的类型（新功能/修复/重构）
   - 确保 .env 等敏感文件不被提交
   - 总结变更的目的（why 而非 what）

3. 创建提交：
   - 添加相关文件（避免 git add -A）
   - 使用 HEREDOC 格式确保提交消息格式：
     git commit -m "$(cat <<'EOF'
     简洁的提交消息（1-2 句话）
     
     解释为什么做这个变更
     EOF
     )"
   - 运行 git status 验证提交成功

4. 如果 pre-commit hook 失败：
   - 修复问题
   - 创建新的 commit（不要用 --amend）

注意事项：
- 绝不跳过 hooks（--no-verify）
- 总是创建新 commit（不用 --amend）
- 不提交敏感文件（.env、credentials）"#;
```

---

## 五、实施优先级

### 高优先级（立即实施）

1. **方案 A：改进工具描述截断逻辑**
   - 位置：`core/src/tools/mod.rs:163-173`
   - 影响：Agent 能看到关键信息
   - 工作量：小（10 行代码）
   - 风险：低

2. **方案：调整提示词注入顺序**
   - 位置：`core/src/prompt.rs:649-722`
   - 影响：Agent 在看工具列表前就知道规则
   - 工作量：小（移动代码位置）
   - 风险：低

### 中优先级（近期实施）

3. **方案 B：分类显示工具**
   - 位置：`core/src/tools/mod.rs`
   - 影响：Agent 明确知道哪些工具优先
   - 工作量：中（50 行代码）
   - 风险：低

4. **Skill/Workflow 触发优化**
   - 位置：新建 `core/src/trigger.rs`
   - 影响：提高触发准确性
   - 工作量：中（需要设计检测逻辑）
   - 风险：中

### 低优先级（可选）

5. **Git 操作优化**
   - 位置：`core/src/prompt.rs`
   - 影响：提高 Git 操作质量
   - 工作量：小
   - 风险：低

6. **冗余标记去除**
   - 位置：`core/src/tools/codegraph/tools.rs`
   - 影响：减少 token 浪费
   - 工作量：小
   - 风险：极低

---

## 六、验证方案

### 6.1 定量验证

#### 测试案例

**案例 1：查找函数定义**
```
用户："查找 Agent 类的定义"
期望：code_search "Agent"
实际：观察工具调用
```

**案例 2：查找调用关系**
```
用户："谁调用了 run 方法"
期望：code_callers "run"
实际：观察工具调用
```

**案例 3：搜索错误信息**
```
用户："查找 'failed to connect' 错误"
期望：grep "failed to connect"
实际：观察工具调用
```

#### 评估指标

**优化前**：
- code_search 使用率：< 10%
- grep 误用率：> 40%

**优化后目标**：
- code_search 使用率：> 50%
- grep 误用率：< 10%

---

### 6.2 定性验证

#### Agent 行为观察

**优化前**：
- Agent 经常先用 grep 查找函数定义
- Agent 不知道 code_search 比 grep 快
- Agent 忽略 [优先] 标记

**优化后期望**：
- Agent 优先使用 code_search 查找符号
- Agent 明确知道性能优势
- Agent 遵循 [优先] 标记

---

## 七、总结

### Claude Code 的优势

1. **详细的工具使用指引**：每个工具都有详细的适用场景说明
2. **丰富的示例**：包含具体的命令示例
3. **Git 操作指引**：详细的 commit/PR 创建步骤

### MatrixCode 的优势

1. **统一的工具选择决策链**：明确的三步验证流程
2. **优先级标记系统**：`is_priority` 字段 + `[优先]` 标记
3. **系统化的调试策略**：专门的 DEBUGGING 模块
4. **清晰的风险分级**：低/中/高三层风险管理
5. **Skill/Workflow 系统**：强制触发机制

### MatrixCode 需要改进的地方

1. **工具描述截断**：关键信息丢失
2. **提示词注入顺序**：规则在工具列表之后
3. **冗余标记**：双重 [优先] 标记

### 推荐实施方案

**立即实施**：
1. 改进工具描述截断逻辑（优先工具保留 150 字符）
2. 调整提示词注入顺序（CODEGRAPH 规则在工具列表之前）

**近期实施**：
3. 分类显示工具（优先工具独立分类）
4. Skill/Workflow 自动触发检测

**可选实施**：
5. Git 操作详细指引
6. 去除冗余标记

---

## 八、附录：工具描述对比示例

### code_search 工具

#### Claude Code 风格
```
搜索代码符号（函数、类、方法、变量）。

适用场景：
- 查找函数/类/变量定义
- 查看符号签名和文档

不适用���景：
- 搜索错误消息
- 查找注释内容
- 搜索字符串常量

性能：比 grep 快 10-100 倍
```

#### MatrixCode 当前（被截断）
```
[优先] 搜索代码符号（函数、类、方法、变量）...
```

#### MatrixCode 优化后
```
【优先工具】
code_search: [优先] 搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍。返回符号位置、签名、文档。grep 仅用于搜索字符串内容（如错误消息）。
```

---

## 九、实施代码示例

### 9.1 工具描述优化

```rust
// core/src/tools/mod.rs

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
    
    let mut lines = vec!["可用工具：".to_string()];
    
    // 优先工具（完整描述）
    if !priority_tools.is_empty() {
        lines.push("\n【优先工具 - 必须优先考虑】".to_string());
        for def in priority_tools {
            // 保留完整描述，包含适用场景
            lines.push(format!("  {}: {}", def.name, def.description));
        }
    }
    
    // 其他工具（简要描述）
    if !normal_tools.is_empty() {
        lines.push("\n【其他工具】".to_string());
        for def in normal_tools {
            let brief = def.description.split('.').next()
                .or_else(|| def.description.split('\n').next())
                .unwrap_or(&def.description);
            let brief = if brief.len() > 60 {
                format!("{}...", brief.chars().take(57).collect::<String>())
            } else {
                brief.to_string()
            };
            lines.push(format!("  {}: {}", def.name, brief));
        }
    }
    
    lines.join("\n")
}
```

### 9.2 提示词注入顺序优化

```rust
// core/src/prompt.rs

pub fn build_system_prompt_with_workflows(
    config: &PromptConfig,
    project_path: Option<&PathBuf>,
    project_context: Option<String>,
    skills: &[Skill],
    workflows: &[Workflow],
    memory_context: Option<String>,
) -> String {
    let mut parts = vec![];
    
    // 1. 静态部分
    let static_prompt = build_static_prompt(config);
    parts.push(static_prompt);
    
    // 🎯 CodeGraph 实践指南（在工具列表之前）
    if let Some(path) = project_path
        && crate::tools::codegraph::should_inject_codegraph_tools(path) {
        parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
    }
    
    // 2. 工具列表
    let tools_prompt = crate::tools::generate_tools_prompt_with_path(project_path);
    parts.push(tools_prompt);
    
    // 3. 项目上下文
    if let Some(context) = project_context {
        parts.push(format!("[PROJECT CONTEXT]\n{}", context));
    }
    
    // 4. Skills
    if !skills.is_empty() {
        parts.push(format!("[AVAILABLE SKILLS]\n{}", 
            skills.iter()
                .map(|s| format!("- /{}: {}", s.name, s.description))
                .join("\n")
        ));
    }
    
    // 5. Workflows
    if !workflows.is_empty() {
        parts.push(format!("[AVAILABLE WORKFLOWS]\n{}",
            workflows.iter()
                .map(|w| format!("- {}: {}", w.id, w.description))
                .join("\n")
        ));
    }
    
    // 6. 记忆上下文
    if let Some(memory) = memory_context {
        parts.push(format!("[ACCUMULATED MEMORY]\n{}", memory));
    }
    
    parts.join("\n\n")
}

// 新增常量
const SYSTEM_PROMPT_CODEGRAPH_PRACTICE: &str = r#"【CodeGraph 工具使用实践】

当前项目已启用 CodeGraph 索引，以下是最佳实践：

查找代码符号：
- 函数/类/变量定义 → code_search（优先，快 10-100 倍）
- 调用关系分析 → code_callers/callees（优先）
- 错误消息/注释 → grep
- 完整文件内容 → read

常见错误纠正：
❌ grep "function_name" → ✅ code_search "function_name"
❌ grep "who calls this" → ✅ code_callers "symbol_id"
❌ read 逐行查找 → ✅ code_search 直接定位

注意：CodeGraph 仅用于代码符号分析，文本内容搜索仍使用 grep。"#;
```

---

## 十、结论

通过对 Claude Code 和 MatrixCode 提示词的深入对比分析，我们发现：

1. **MatrixCode 的优势**在于系统化的决策链、优先级标记和风险管理，但在工具描述传递上存在关键问题。

2. **核心问题**是工具描述被截断导致关键信息丢失，Agent 看不到"必须优先使用"等指引。

3. **推荐优化**包括：
   - 改进工具描述截断逻辑（优先工具保留完整描述）
   - 调整提示词注入顺序（CODEGRAPH 规则在工具列表之前）
   - 分类显示工具（优先工具独立���类）

4. **预期效果**：code_search 使用率从 < 10% 提升到 > 50%，grep 误用率从 > 40% 降低到 < 10%。

这些优化将显著提升 Agent 的工具选择准确性，充分发挥 CodeGraph 的性能优势。