# Skills 和 Workflows 提示词改进方案

## 🔴 **当前问题分析**

### **位置问题**

当前顺序：
```
1. IDENTITY (身份)
2. TOOL_DECISION (工具决策)
3. SKILLS (技能系统) ← 位置靠后
4. WORKFLOWS (工作流系统) ← 位置靠后
5. MISSION (核心目标)
6. WORKFLOW (工作方式)
...
```

**问题**: Skills 和 Workflows 是 MatrixCode 的核心特性，但在提示词中的位置不够突出。

---

### **描述问题**

当前描述：
```rust
【Skills 系统】

Skills 是可加载的专业指令模块，用于特定场景的最佳实践。

使用方式：
1. 查看可用 Skills → 在 [AVAILABLE SKILLS] 部分查找匹配触发条件的 skill
2. 调用 Skill → 使用 `skill` 工具，传入 skill 名称
3. Skill 加载后 → 完整指令会被注入，指导后续行为
```

**问题**:
1. ❌ 描述过于温和（"可加载"、"用于特定场景"）
2. ❌ 缺少优先级标记（没有 `[优先]` 标记）
3. ❌ 没有强调核心特性地位
4. ❌ 缺少具体触发场景示例

---

### **触发逻辑问题**

当前触发：
```rust
最佳实践：
- 阻塞要求：当用户请求匹配 skill 触发条件时，必须在生成其他响应前调用
```

**问题**:
1. ❌ "匹配触发条件"不够明确
2. ❌ 缺少自动触发机制
3. ❌ 没有优先级指导
4. ❌ 缺少降级处理方案

---

## 🎯 **改进方案**

### **改进 1: 位置调整**

**新顺序**:
```
1. IDENTITY (身份) ← 提到核心特性
2. SKILLS (技能系统) ← 提前到第2位 ✅
3. WORKFLOWS (工作流系统) ← 提前到第3位 ✅
4. TOOL_DECISION (工具决策) ← 移到后面
5. MISSION (核心目标)
6. WORKFLOW (工作方式)
...
```

**理由**:
- Skills 和 Workflows 在 IDENTITY 中被列为核心特性，应该紧随其后详细介绍
- AI 在做工具选择决策前，应该先了解这两个优先级最高的系统
- 与 CodeGraph/LSP 的动态注入逻辑一致（先介绍系统，再介绍工具）

---

### **改进 2: 描述增强**

#### **Skills 新描述**

```rust
const SYSTEM_PROMPT_SKILLS: &str = r#"【Skills 技能系统 - 核心特性】

Skills 是 MatrixCode 的核心特性之一，提供场景化的最佳实践指导。

🔴 **重要程度**: 最高优先级 - 遇到匹配场景必须优先调用

【触发机制 - 自动识别】

以下情况必须先调用 Skill：
- 用户说 "/review" 或 "审查代码" → 调用 "code-review" skill
- 用户说 "/refactor" 或 "重构代码" → 调用 "refactor" skill
- 用户说 "/debug" 或 "调试问题" → 调用 "debugging" skill
- 用户说 "/plan" 或 "规划方案" → 调用 "planning" skill
- 用户提到特定领域（安全、性能、测试）→ 查找对应 skill

【强制执行规则】

1. **阻塞调用**: 发现匹配场景时，必须在生成任何其他响应前调用 skill 工具
2. **不要提及**: 不要在文本中提及 skill 名称而不实际调用
3. **不要重复**: 看到输出中有 <command-name> 标签表示已加载，不要再调用
4. **立即执行**: skill 返回后立即执行其中的指令，不要等待用户确认

【调用示例】

正确做法：
用户: "审查这段代码的安全性"
AI: 
  → 调用 skill {"name": "security-review"}  ← 阻塞调用
  → 返回指令："检查用户输入验证、SQL 注入、XSS..."
  → 立即执行指令，调用 code_search 查找用户输入处理代码
  → 生成审查报告

错误做法：
用户: "审查这段代码的安全性"
AI: "我来审查代码的安全性..." ← 错误：未先调用 skill
  → 应该先调用 security-review skill

【工具用法】

调用方式：
{"name": "skill", "arguments": {"name": "skill-name"}}

查看可用 skills：
查看系统提示词末尾的 [AVAILABLE SKILLS] 部分"#;
```

---

#### **Workflows 新描述**

```rust
const SYSTEM_PROMPT_WORKFLOWS: &str = r#"【Workflows 工作流系统 - 核心特性】

Workflows 是 MatrixCode 的核心特性之一，提供自动化多步骤任务执行。

🔴 **重要程度**: 最高优先级 - 复杂任务必须优先考虑 workflow

【触发机制 - 自动识别】

以下情况必须先考虑 Workflow：
- 用户请求包含多个步骤（"分析、审查、生成文档"）
- 用户请求研究型任务（"搜索多个来源、汇总信息"）
- 用户请求批量操作（"处理所有文件"）
- 用户请求生成报告（"生成项目分析报告"）
- 用户请求自动化流程（"自动化部署流程"）

【强制执行规则】

1. **优先检查**: 遇到复杂任务时，必须先用 workflow_discover 查找是否有匹配 workflow
2. **优先调用**: 如果有匹配 workflow，优先使用 workflow_run 而非手动执行多个步骤
3. **参数验证**: 必须提供 required_inputs 中列出的所有参数
4. **执行监控**: workflow 执行过程中不要中断，等待完成后再继续

【调用示例】

正确做法：
用户: "生成一份 Rust 性能优化文章，包含图片和代码示例"
AI:
  → 调用 workflow_discover 查找匹配 workflow  ← 优先检查
  → 发现 "image-article" workflow 匹配
  → 调用 workflow_run {"workflow_id": "image-article", "inputs": {"topic": "Rust 性能优化"}}
  → Workflow 自动执行：搜索图片 → 生成内容 → 格式化输出
  → 返回结果："已生成文章..."

错误做法：
用户: "生成一份 Rust 性能优化文章，包含图片和代码示例"
AI: "我先搜索图片..." ← 错误：未先检查 workflow
  → 应该先调用 workflow_discover

【Workflow 特性】

- 声明式配置：步骤、条件、循环都在 YAML 中定义
- 可组合：workflow 可以调用其他 workflow 或 skill
- 后台运行：支持后台执行，完成后通知
- 项目级 + 用户级：.matrix/workflows/ 和 ~/.matrix/workflows/ 都会被扫描

【工具用法】

查看可用 workflows：
{"name": "workflow_discover", "arguments": {}}

执行 workflow：
{"name": "workflow_run", "arguments": {"workflow_id": "workflow-id", "inputs": {...}}}

匹配 workflow：
{"name": "workflow_match", "arguments": {"intent": "用户意图描述"}}"#;
```

---

### **改进 3: 触发逻辑强化**

#### **新增：强制触发检测模块**

```rust
const SYSTEM_PROMPT_TRIGGER_LOGIC: &str = r#"【强制触发检测 - 执行前必查】

在生成任何响应前，必须执行以下检测流程：

🔴 Step 1: Skill 触发检测
检查用户请求是否匹配以下关键词：
- "/review", "审查", "检查代码" → skill("code-review")
- "/refactor", "重构", "优化结构" → skill("refactor")
- "/debug", "调试", "排查问题" → skill("debugging")
- "/plan", "规划", "设计方案" → skill("planning")
- "/security", "安全审查" → skill("security-review")

如果匹配 → 立即调用 skill 工具，不要生成任何文本响应

🔴 Step 2: Workflow 触发检测
检查用户请求是否包含以下特征：
- 多个步骤关键词（"分析、审查、生成"）
- 研究型任务（"搜索、汇总"）
- 批量操作（"处理所有、批量"）
- 报告生成（"生成报告、文档"）

如果匹配 → 立即调用 workflow_discover 或 workflow_match

🔴 Step 3: 工具选择决策
如果以上都不匹配 → 继续执行工具选择决策链（TOOL_DECISION）

【执行顺序强制要求】
Skill/Workflow 触发检测 → 工具选择决策 → 生成响应
绝对不要跳过前两步！"#;
```

---

### **改进 4: 模块顺序调整**

```rust
const DEFAULT_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,        // 1. MatrixCode 身份 + 特性
    SYSTEM_PROMPT_SKILLS,          // 2. Skills 系统（核心特性）← 提前 ✅
    SYSTEM_PROMPT_WORKFLOWS,       // 3. Workflows 系统（核心特性）← 提前 ✅
    SYSTEM_PROMPT_TRIGGER_LOGIC,   // 4. 强制触发检测 ← 新增 ✅
    SYSTEM_PROMPT_TOOL_DECISION,   // 5. 工具选择决策链 ← 移后
    SYSTEM_PROMPT_MISSION,         // 6. 核心目标
    SYSTEM_PROMPT_WORKFLOW,        // 7. 工作方式
    SYSTEM_PROMPT_AMBIGUITY,       // 8. 歧义确认
    SYSTEM_PROMPT_BEHAVIOR,        // 9. 行为约束
    SYSTEM_PROMPT_RISK_MANAGEMENT, // 10. 操作风险分级
    SYSTEM_PROMPT_GIT_SAFETY,      // 11. Git Safety Protocol
    SYSTEM_PROMPT_SYSTEM_RULES,    // 12. 系统规则
    SYSTEM_PROMPT_QUALITY,         // 13. 代码质量
    SYSTEM_PROMPT_TESTING,         // 14. 测试验证
    SYSTEM_PROMPT_DEBUGGING,       // 15. 调试策略
    SYSTEM_PROMPT_SECURITY,        // 16. 安全意识
    SYSTEM_PROMPT_EDITING,         // 17. 编辑规则
    SYSTEM_PROMPT_EXECUTION,       // 18. 执行策略
    SYSTEM_PROMPT_LANGUAGE,        // 19. 语言规则
    SYSTEM_PROMPT_OUTPUT_CONTROL,  // 20. 输出控制
    SYSTEM_PROMPT_COMPLETION,      // 21. 完成要求
    SYSTEM_PROMPT_TASK_TRACKING,   // 22. 任务追踪
];
```

---

## 📊 **改进效果对比**

### **Token 使用**

| 模块 | 旧版本 | 新版本 | 增加 |
|------|--------|--------|------|
| SKILLS | ~180 token | ~400 token | +220 |
| WORKFLOWS | ~180 token | ~400 token | +220 |
| TRIGGER_LOGIC | 0 | ~200 token | +200 |
| **总计** | ~360 token | ~1000 token | +640 |

**理由**: 增强的描述和触发逻辑能够显著提高 AI 的正确使用率，值得额外的 token 成本。

---

### **AI 行为改进**

#### **旧版本行为** ❌

```
用户: "审查这段代码的安全性"

AI: "我来审查代码的安全性..."
    → 调用 code_search 查找代码
    → 手动分析安全问题
    → 生成报告

问题: 未使用 security-review skill，可能遗漏关键检查项
```

#### **新版本行为** ✅

```
用户: "审查这段代码的安全性"

AI: 
    → 检测触发："安全审查" 匹配 skill 触发条件
    → 调用 skill {"name": "security-review"}  ← 强制触发
    → 返回指令："检查用户输入验证、SQL 注入、XSS、CSRF..."
    → 按指令逐项检查
    → 生成完整安全审查报告

优势: 使用专业 skill，确保检查完整性
```

---

## 🧪 **测试验证**

### **单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_skills_module_position() {
        let modules = DEFAULT_SYSTEM_PROMPT_MODULES;
        assert_eq!(modules[0], SYSTEM_PROMPT_IDENTITY);
        assert_eq!(modules[1], SYSTEM_PROMPT_SKILLS); // 第2位
        assert_eq!(modules[2], SYSTEM_PROMPT_WORKFLOWS); // 第3位
    }
    
    #[test]
    fn test_skills_includes_trigger_keywords() {
        assert!(SYSTEM_PROMPT_SKILLS.contains("/review"));
        assert!(SYSTEM_PROMPT_SKILLS.contains("/refactor"));
        assert!(SYSTEM_PROMPT_SKILLS.contains("阻塞调用"));
    }
    
    #[test]
    fn test_workflows_includes_trigger_keywords() {
        assert!(SYSTEM_PROMPT_WORKFLOWS.contains("多个步骤"));
        assert!(SYSTEM_PROMPT_WORKFLOWS.contains("研究型任务"));
        assert!(SYSTEM_PROMPT_WORKFLOWS.contains("优先检查"));
    }
}
```

---

## 📅 **实现计划**

### **Week 1: 描述增强**
- 重写 SYSTEM_PROMPT_SKILLS
- 重写 SYSTEM_PROMPT_WORKFLOWS
- 添加 SYSTEM_PROMPT_TRIGGER_LOGIC

### **Week 2: 位置调整**
- 更新 DEFAULT_SYSTEM_PROMPT_MODULES
- 更新 SAFE/FAST/REVIEW profiles
- 运行测试验证

### **Week 3: 效果验证**
- 测试 AI 触发行为
- 统计 skill/workflow 使用率
- 收集用户反馈

---

## ✅ **总结**

**关键改进**:

1. ✅ **位置提前**: Skills 和 Workflows 从第3-4位提前到第2-3位
2. ✅ **描述增强**: 使用更强烈的语言和具体示例
3. ✅ **触发强化**: 添加明确的触发关键词和强制检测流程
4. ✅ **新增模块**: SYSTEM_PROMPT_TRIGGER_LOGIC 确保执行前必查

**预期效果**:
- AI 自动识别 skill/workflow 触发场景
- 优先使用 skill/workflow 而非手动执行
- 提高任务完成质量和效率