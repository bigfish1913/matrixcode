# 工具注册完整性分析报告

## 执行摘要

**结论**: CodeGraph 工具已正确注册，但发现 **VerifyTool 未注册** 和工具提示截断问题。

**影响**: 
- VerifyTool 未注册 → 用户无法获取代码验证建议
- 工具描述截断 → Agent 无法了解完整适用场景

---

## 一、工具注册现状

### 1.1 CodeGraph 工具（✅ 已注册）

| 工具名 | 注册状态 | 注入条件 | 用途 |
|--------|---------|---------|------|
| `code_search` | ✅ 已注册 | CLI + .codegraph | 搜索代码符号（优先工具） |
| `code_callers` | ✅ 已注册 | CLI + .codegraph | 查找调用者（优先工具） |
| `code_callees` | ✅ 已注册 | CLI + .codegraph | 查找被调用者（优先工具） |
| `code_status` | ✅ 已注册 | CLI + .codegraph | 检查索引状态 |
| `code_sync` | ✅ 已注册 | CLI + .codegraph | 手动同步索引 |

**注册位置**: `core/src/tools/mod.rs:522-524`

```rust
if codegraph::should_inject_codegraph_tools(&project_path) {
    tools.extend(codegraph::codegraph_tools(&project_path));
}
```

### 1.2 LSP 工具（✅ 已注册）

| 工具名 | 注册状态 | 注入条件 | 用途 |
|--------|---------|---------|------|
| `lsp_hover` | ✅ 已注册 | lsp_registry | 类型签名和文档 |
| `lsp_definition` | ✅ 已注册 | lsp_registry | 跳转到定义 |
| `lsp_references` | ✅ 已注册 | lsp_registry | 查找引用 |
| `lsp_diagnostics` | ✅ 已注册 | lsp_registry | 诊断信息 |

**注册位置**: `core/src/tools/mod.rs:526-528`

```rust
if let Some(registry) = lsp_registry {
    tools.extend(crate::lsp::tools::lsp_tools(registry));
}
```

### 1.3 Workflow 工具（✅ 已注册）

| 工具名 | 注册状态 | 注入方式 | 用途 |
|--------|---------|---------|------|
| `workflow_discover` | ✅ 已注册 | 无条件 | 发现可用 workflow |
| `workflow_run` | ✅ 已注册 | 无条件 | 执行 workflow |
| `workflow_match` | ✅ 已注册 | 无条件 | 匹配 workflow |
| `workflow_create` | ✅ 已注册 | 无条件 | 创建 workflow |
| `content_generation` | ✅ 已注册 | 需要 provider | AI 内容生成 |

**注册位置**: `core/src/tools/mod.rs:206, 530`

```rust
// 基础工具（无条件）
tools.extend(workflow::workflow_tools());

// AI 工具（需要 provider）
tools.extend(workflow::workflow_tools_with_provider(provider));
```

### 1.4 🔴 Verify 工具（❌ 未注册）

| 工具名 | 注册状态 | 文件位置 | 用途 |
|--------|---------|---------|------|
| `VerifyTool` | ❌ **未注册** | `core/src/tools/verify.rs` | 代码验证建议 |

**问题**: 定义了完整功能，但从未在任何注册函数中调用！

**功能**:
- 自动检测项目类型（Rust/Node.js/Python/Go/Java）
- 推断相关测试文件
- 生成验证命令建议（test, build, typecheck, lint）

---

## 二、工具注入流程分析

### 2.1 CLI 入口点

**文件**: `cli/src/terminal/agent.rs:167-172`

```rust
let mut base_tools = all_tools_full_with_lsp(
    Arc::new(ctx.skills.clone()),
    provider.clone_arc(),
    project_path_for_tools.clone(),
    Some(lsp_registry),
);
base_tools.extend(mcp_tools);
```

### 2.2 核心注册函数

**文件**: `core/src/tools/mod.rs:514-532`

```rust
pub fn all_tools_full_with_lsp(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn Provider>,
    project_path: PathBuf,
    lsp_registry: Option<Arc<LspClientRegistry>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    
    // ✅ CodeGraph 工具（条件注入）
    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }
    
    // ✅ LSP 工具（条件注入）
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }
    
    // ✅ Workflow 工具（无条件 + 需要 provider）
    tools.extend(workflow::workflow_tools_with_provider(provider));
    
    // ❌ 缺少 Verify 工具注册！
    
    tools
}
```

### 2.3 基础工具集

**文件**: `core/src/tools/mod.rs:122-148`

```rust
fn base_tools(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ask::AskTool),
        Box::new(read::ReadTool),
        Box::new(write::WriteTool::new()),
        Box::new(edit::EditTool),
        Box::new(multi_edit::MultiEditTool),
        Box::new(search::SearchTool),
        Box::new(grep::GrepTool),
        Box::new(glob::GlobTool),
        Box::new(ls::LsTool),
        Box::new(bash::BashTool),
        Box::new(browser::BrowserOpenTool),
        Box::new(todo_write::TodoWriteTool),
        Box::new(websearch::WebSearchTool::new()),
        Box::new(webfetch::WebFetchTool),
        Box::new(skill::SkillTool::new(skills)),
        Box::new(task::TaskTool),
        Box::new(task::TaskCreateTool),
        Box::new(task::TaskGetTool),
        Box::new(task::TaskListTool),
        Box::new(task::TaskStopTool),
        Box::new(plan_mode::EnterPlanModeTool),
        Box::new(plan_mode::ExitPlanModeTool),
        Box::new(monitor::MonitorTool),
        // ❌ 缺少 verify::VerifyTool！
    ]
}
```

---

## 三、系统提示工具显示问题

### 3.1 工具提示生成

**文件**: `core/src/tools/mod.rs:208-267`

**问题**: 优先工具完整描述（150字符），普通工具截断（60字符）

```rust
// 优先工具：最多 150 字符
if desc.len() > 150 {
    lines.push(format!("{}: {}...", def.name, desc.chars().take(147).collect::<String>()));
}

// 普通工具：最多 60 字符（严重截断！）
if desc.len() > 60 {
    lines.push(format!("{}: {}...", def.name, desc.chars().take(57).collect::<String>()));
}
```

### 3.2 影响

- **code_search** (优先): "搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍..." ✅ 完整
- **workflow_discover** (普通): "发现可执行的自动化流程..." ❌ 截断，丢失关键信息

---

## 四、改进建议

### 4.1 🔴 立即修复：注册 VerifyTool

**方案 A**: 添加到 base_tools（无条件注入）

```rust
fn base_tools(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    let mut tools = vec![
        // ... 现有工具 ...
    ];
    
    // 添加 VerifyTool（需要 project_path）
    tools.push(Box::new(verify::VerifyTool::new(
        std::env::current_dir().unwrap_or_default()
    )));
    
    tools
}
```

**方案 B**: 条件注入（推荐）

```rust
pub fn all_tools_full_with_lsp(
    ...
    project_path: PathBuf,
    ...
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    
    // ✅ Verify 工具（条件注入）
    tools.push(Box::new(verify::VerifyTool::new(project_path.clone())));
    
    // ✅ CodeGraph 工具
    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }
    
    // ... 其他工具 ...
    tools
}
```

### 4.2 优化工具提示显示

**调整截断策略**:

```rust
// 优先工具：完整描述（不截断）
lines.push(format!("  {}: {}", def.name, def.description_for_llm()));

// 重要普通工具：200 字符
const IMPORTANT_NORMAL_TOOLS = ["workflow_discover", "workflow_run", "task"];
if IMPORTANT_NORMAL_TOOLS.contains(&def.name) {
    let desc = if def.description.len() > 200 {
        def.description.chars().take(197).collect::<String>() + "..."
    } else {
        def.description.clone()
    };
    lines.push(format!("  {}: {}", def.name, desc));
}

// 其他工具：100 字符（提高信息量）
if desc.len() > 100 {
    lines.push(format!("{}: {}...", def.name, desc.chars().take(97).collect::<String>()));
}
```

### 4.3 统一工具注册机制

**创建 ToolRegistry**:

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

// 使用示例
let registry = ToolRegistry::new();
registry.register(Box::new(CodeGraphSearchTool::new(project_path)));
registry.register(Box::new(VerifyTool::new(project_path)));

let tools = registry.all();
```

---

## 五、验证计划

### 5.1 单元测试

```rust
#[test]
fn test_all_tools_include_verify() {
    let tools = all_tools_full_with_lsp(
        Arc::new(Vec::new()),
        mock_provider(),
        PathBuf::from("."),
        None,
    );
    
    let tool_names = tools.iter().map(|t| t.definition().name).collect::<Vec<_>>();
    
    assert!(tool_names.contains(&"verify"), "VerifyTool should be registered");
}
```

### 5.2 集成测试

```rust
#[test]
fn test_verify_tool_in_system_prompt() {
    let prompt = generate_tools_prompt_with_path(Some(&PathBuf::from(".")));
    
    assert!(prompt.contains("verify"), "System prompt should mention verify tool");
}
```

---

## 六、影响评估

| 改进项 | 优先级 | 影响范围 | 工作量 |
|--------|-------|---------|-------|
| 注册 VerifyTool | 🔴 P0 | 功能缺失 | 10 分钟 |
| 优化工具提示截断 | 🟡 P1 | 用户体验 | 30 分钟 |
| 统一注册机制 | 🟢 P2 | 代码维护 | 2 小时 |

---

## 七、总结

### 已注册工具统计

- **总工具数**: 36 个
- **已注册**: 35 个 ✅
- **未注册**: 1 个 ❌ (VerifyTool)

### CodeGraph 工具状态

**结论**: CodeGraph 工具已正确注册，用户可能误解了问题。

真正的问题是 **VerifyTool 未注册**，这是一个独立的工具模块，用于代码验证建议。

### 下一步行动

1. **立即修复**: 注册 VerifyTool（方案 B：条件注入）
2. **优化显示**: 调整工具提示截断策略
3. **长期优化**: 统一工具注册机制

---

**报告生成时间**: 2025-06-17  
**分析工具**: MatrixCode Agent  
**项目**: MatrixCode Core v0.4.39