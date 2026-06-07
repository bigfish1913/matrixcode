# CodeGraph 工具注册分析报告

## 执行时间
2026-06-17

## 分析目标
检查 CodeGraph 相关工具是否已正确注册到 Agent，并发现其他可能遗漏的工具。

---

## 一、工具注册现状

### 1. CodeGraph 工具 ✅ 已完整注册

**工具位置**: `core/src/tools/codegraph/tools.rs`

**已定义工具** (5 个):
- `code_search` - 搜索代码符号（优先工具，比 grep 快 10-100 倍）
- `code_callers` - 查找调用者（优先工具）
- `code_callees` - 查找被调用者（优先工具）
- `code_status` - 检查索引状态
- `code_sync` - 手动同步索引

**注册位置**: `core/src/tools/mod.rs:522-524`
```rust
if codegraph::should_inject_codegraph_tools(&project_path) {
    tools.extend(codegraph::codegraph_tools(&project_path));
}
```

**注入条件**:
- CodeGraph CLI 已安装
- 项目存在 `.codegraph` 目录

**系统提示显示**: ✅ 正常（通过 `generate_tools_prompt_with_path_and_lsp`）

---

### 2. LSP 工具 ✅ 已完整注册

**工具位置**: `core/src/lsp/tools.rs`

**已定义工具** (4 个):
- `lsp_hover` - 获取类型签名和文档
- `lsp_definition` - 跳转到定义位置
- `lsp_references` - 查找所有引用
- `lsp_diagnostics` - 获取诊断信息（错误、警告）

**注册位置**: `core/src/tools/mod.rs:526-528`
```rust
if let Some(registry) = lsp_registry {
    tools.extend(crate::lsp::tools::lsp_tools(registry));
}
```

**注入条件**: LSP registry 已配置（通过 `lsp.toml`）

**系统提示显示**: ✅ 正常

---

### 3. Workflow 工具 ✅ 已完整注册

**工具位置**: `core/src/tools/workflow/`

**已定义工具** (5 个):
- `workflow_discover` - 发现可执行流程
- `workflow_run` - 执行 workflow
- `workflow_match` - 匹配 workflow
- `workflow_create` - 创建和编辑 workflow
- `content_generation` - AI 内容生成（需要 provider）

**注册位置**: `core/src/tools/mod.rs:530`
```rust
tools.extend(workflow::workflow_tools_with_provider(provider));
```

**注入条件**: 无条件注入（基础工具集）

**系统提示显示**: ✅ 正常

---

## 二、发现的遗漏工具

### 🔴 VerifyTool - 未注册

**文件位置**: `core/src/tools/verify.rs` (17KB, 385 行)

**工具功能**:
- 自动检测项目类型（Rust/Node.js/Python/Go/Java）
- 推断相关测试文件
- 生成验证建议（type check、lint、test、build）

**设计意图**:
根据文件名和项目类型，推断应该运行的验证命令，帮助用户确保代码质量。

**使用场景**:
```rust
// 示例：修改 Rust 文件后
let suggestion = generate_verify_suggestion(&project_root, "src/utils.rs");
// 输出：
// - TypeCheck: cargo check
// - Lint: cargo clippy  
// - Test: cargo test --test utils_test (如果存在 tests/utils_test.rs)
// - Build: cargo build
```

**遗漏原因**:
- 文件存在但未在 `base_tools()` 中注册
- 可能是遗留的未完成功能

**建议处理方式**:

#### 方案 A：注册为 Tool（推荐）
将其作为普通工具注册，允许 Agent 在代码修改后主动调用。

**优点**:
- Agent 可以主动建议验证步骤
- 符合现有工具架构

**缺点**:
- 可能增加 Agent 决策负担

#### 方案 B：集成到 Edit/Write Hook
将其集成到 `edit` 和 `write` 工具的后置 hook 中，自动输出验证建议。

**优点**:
- 无需 Agent 主动调用
- 自动化验证建议流程

**缺点**:
- 需要修改 hook 系统

#### 方案 C：作为独立命令
将其作为 CLI 命令而非 Agent 工具，用户手动调用。

**优点**:
- 不干扰 Agent 工作流程
- 用户可按需调用

**缺点**:
- 无法自动建议验证

**推荐**: **方案 B** - 集成到 Edit/Write Hook，实现自动化验证建议。

---

## 三、工具注入流程分析

### 当前注入架构

```
用户启动 Agent
    ↓
CLI: terminal/agent.rs:run_agent_task()
    ↓
构建工具集: all_tools_full_with_lsp()
    ├─ base_tools() - 基础工具（22 个）
    ├─ CodeGraph 工具（条件注入）
    ├─ LSP 工具（条件注入）
    └─ Workflow 工具（无条件）
    ↓
传递给 AgentBuilder
    ↓
Agent 执行时调用工具
```

### 系统提示生成流程

```
build_system_prompt_with_workflows_and_lsp()
    ├─ 基础提示（静态 section）
    ├─ 工具提示: generate_tools_prompt_with_path_and_lsp()
    │   ├─ 分类：优先工具 vs 普通工具
    │   ├─ 截断：优先工具 150 字符，普通工具 60 字符
    │   └─ 格式化输出
    ├─ Skills 提示
    ├─ Workflows 提示
    ├─ Project Overview
    ├─ Memory Summary
    └─ LSP Servers 状态
```

---

## 四、潜在改进建议

### 1. 工具描述截断问题

**现状**:
- 优先工具：最多 150 字符
- 普通工具：最多 60 字符

**问题**:
部分工具的详细使用场景可能被截断，导致 Agent 不清楚何时使用。

**建议**:
- 保持优先工具完整描述（不截断）
- 普通工具保留关键信息（第一句 + 适用场景）

### 2. VerifyTool 集成方案

**推荐实现**（方案 B）:

```rust
// core/src/tools/tool_hooks.rs
pub fn post_edit_hook(file_path: &str, result: &str) -> Option<String> {
    let project_root = detect_project_root(file_path);
    let verify = VerifyTool::new(project_root);
    let suggestion = verify.generate_suggestion(file_path);
    
    if !suggestion.commands.is_empty() {
        let commands = suggestion.commands.iter()
            .map(|c| format!("  • {}: {}", c.kind, c.command))
            .join("\n");
        
        return Some(format!(
            "\n\n💡 建议验证步骤：\n{}",
            commands
        ));
    }
    None
}
```

### 3. CodeGraph 工具文档优化

**现状**: CodeGraph 工具标记为优先工具，但 Agent 可能不清楚具体使用场景。

**建议**: 在系统提示中添加明确的使用指南：

```markdown
【工具优先级决策链】

第1步：判断意图
- 找代码定义？ → code_search（优先，快 10-100 倍）
- 搜文本内容？ → grep（错误消息、日志、注释）
- 查调用关系？ → code_callers/callees（优先）
- 改文件？ → edit/write
- 执行命令？ → bash

第2步：限制搜索范围
- 先探索结构：ls() → ls({ path: "src" })
- 使用路径参数：grep({ path: "src/api" })
- 使用过滤参数：grep({ type: "rs" })

第3步：验证工具可用性
- 检查工具列表中是否有 code_* 工具
- 有 → 优先使用
- 无 → 使用 grep 替代
```

### 4. 工具注册中心化

**现状**: 工具分散在多个文件中，注册逻辑在 `mod.rs`。

**建议**: 创建 `ToolRegistry` 集中管理：

```rust
// core/src/tools/registry.rs
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.definition().name;
        self.tools.insert(name, tool);
    }
    
    pub fn get(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }
    
    pub fn all(&self) -> Vec<Box<dyn Tool>> {
        self.tools.values().cloned().collect()
    }
}
```

---

## 五、总结

### ✅ 已正确注册的工具

| 类别 | 工具数 | 注册状态 | 注入条件 |
|------|--------|----------|----------|
| 基础工具 | 22 | ✅ | 无条件 |
| CodeGraph | 5 | ✅ | CLI + .codegraph |
| LSP | 4 | ✅ | lsp_registry |
| Workflow | 5 | ✅ | 无条件 |

### 🔴 未注册的工具

| 工具 | 文件大小 | 功能 | 建议处理 |
|------|---------|------|----------|
| VerifyTool | 17KB | 验证建议生成 | 集成到 Edit/Write Hook |

### 📊 工具总数统计

- **已定义**: 36 个工具实现
- **已注册**: 35 个工具
- **未注册**: 1 个工具（VerifyTool）
- **注册率**: 97.2%

---

## 六、下一步行动建议

### 立即执行（高优先级）

1. **集成 VerifyTool** - 将验证建议集成到 Edit/Write 后置 hook
2. **优化工具描述截断** - 调整截断逻辑，保留关键信息

### 后续优化（中优先级）

3. **完善工具决策指南** - 在系统提示中添加明确的工具选择流程
4. **创建工具注册中心** - 集中管理工具注册和查找

### 长期改进（低优先级）

5. **工具使用统计** - 收集工具调用频率，优化工具描述
6. **动态工具注入** - 根据项目类型智能注入相关工具

---

## 附录：工具完整清单

### 基础工具 (base_tools)

1. ask - 向用户提问
2. read - 读取文件
3. write - 写入文件
4. edit - 单处编辑
5. multi_edit - 多处编辑
6. search - 文本搜索
7. grep - 正则搜索
8. glob - 文件查找
9. ls - 列出目录
10. bash - 执行命令
11. browser - 打开浏览器
12. todo_write - 任务追踪
13. websearch - 网络搜索
14. webfetch - 网页抓取
15. skill - 技能调用
16. task - 启动代理
17. task_create - 创建任务
18. task_get - 查询任务
19. task_list - 列出任务
20. task_stop - 停止任务
21. enter_plan_mode - 进入规划
22. exit_plan_mode - 退出规划
23. monitor - 监控进程

### CodeGraph 工具 (条件注入)

24. code_search - 搜索符号 ⭐
25. code_callers - 查找调用者 ⭐
26. code_callees - 查找被调用者 ⭐
27. code_status - 索引状态
28. code_sync - 同步索引

### LSP 工具 (条件注入)

29. lsp_hover - 类型信息
30. lsp_definition - 跳转定义
31. lsp_references - 查找引用
32. lsp_diagnostics - 诊断信息

### Workflow 工具

33. workflow_discover - 发现流程
34. workflow_run - 执行流程
35. workflow_match - 匹配流程
36. workflow_create - 创建流程
37. content_generation - 内容生成

### 未注册工具

38. verify - 验证建议 🔴

---

**报告生成时间**: 2026-06-17  
**分析工具**: code_search, grep, read, ls  
**检查文件数**: 40+  
**发现问题数**: 1 个遗漏工具