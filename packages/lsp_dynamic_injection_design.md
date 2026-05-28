# MatrixCode LSP 动态注入设计方案

## 🎯 设计目标

LSP 功能应该像 CodeGraph 一样采用**动态注入机制**，而不是硬编码到系统提示词中。

---

## 📋 动态注入机制分析

### 当前 CodeGraph 动态注入流程

```rust
// 1️⃣ 工具列表动态生成 (core/src/tools/mod.rs:146)
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));
    
    // 动态添加 CodeGraph 工具（条件：CLI 已安装 + .codegraph 存在）
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }
    
    // 生成工具描述文本
    for tool in tools {
        lines.push(format!("- {}: {}", def.name, brief));
    }
}

// 2️⃣ 系统提示词动态注入 (core/src/prompt.rs:736)
if let Some(path) = project_path
    && crate::tools::codegraph::should_inject_codegraph_tools(path) {
    parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
    parts.push(SYSTEM_PROMPT_CODEGRAPH.to_string());
}

// 3️⃣ 条件检测 (core/src/tools/codegraph/tools.rs:276)
pub fn should_inject_codegraph_tools(start_path: &Path) -> bool {
    super::install::is_codegraph_installed() && 
        CodeGraphManager::with_auto_detect(start_path).is_initialized()
}
```

---

## 🏗️ LSP 动态注入设计

### Phase 1: 条件检测函数

**文件**: `core/src/lsp/mod.rs`

```rust
/// Check if LSP tools should be injected (servers are running)
pub fn should_inject_lsp_tools() -> bool {
    // 检查是否有活跃的 LSP 服务器
    if let Some(manager) = get_global_lsp_manager() {
        !manager.active_servers().is_empty()
    } else {
        false
    }
}

/// Get list of active LSP servers with their capabilities
pub fn get_active_lsp_servers() -> Vec<LspServerInfo> {
    if let Some(manager) = get_global_lsp_manager() {
        manager.active_servers()
    } else {
        vec![]
    }
}
```

---

### Phase 2: 动态工具生成

**文件**: `core/src/tools/lsp.rs` (新建)

```rust
use crate::lsp::{should_inject_lsp_tools, get_active_lsp_servers};

/// Create LSP tools if servers are running
pub fn lsp_tools_if_active() -> Vec<Box<dyn Tool>> {
    if should_inject_lsp_tools() {
        lsp_tools()
    } else {
        vec![]
    }
}

/// LSP tool definitions
pub fn lsp_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(LspGotoDefinitionTool),
        Box::new(LspFindReferencesTool),
        Box::new(LspGetDiagnosticsTool),
        Box::new(LspHoverTool),
    ]
}

// 工具实现示例
pub struct LspGotoDefinitionTool;

impl Tool for LspGotoDefinitionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_goto_definition".to_string(),
            description: "[优先] 跳转到符号定义。使用 LSP 实时分析，比 code_search 更准确。适用场景：查找函数/类/变量的精确定义位置。不适用：LSP 未启动或文件不在项目中。".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file", "line", "character"],
                "properties": {
                    "file": {"type": "string", "description": "文件路径"},
                    "line": {"type": "integer", "description": "行号（从 0 开始）"},
                    "character": {"type": "integer", "description": "列号（从 0 开始）"}
                }
            }),
            is_priority: true,
        }
    }
    
    async fn execute(&self, params: Value) -> Result<String> {
        // 调用 LSP manager 的 goto_definition
        let manager = get_global_lsp_manager()?;
        let file = params["file"].as_str().unwrap();
        let line = params["line"].as_u64().unwrap() as u32;
        let character = params["character"].as_u64().unwrap() as u32;
        
        manager.goto_definition(file, line, character)
    }
}
```

---

### Phase 3: 工具列表动态添加

**文件**: `core/src/tools/mod.rs`

```rust
// 在 generate_tools_prompt_with_path() 中添加
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));
    
    // 动态添加 CodeGraph 工具
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }
    
    // 动态添加 LSP 工具（新增）
    if lsp::should_inject_lsp_tools() {
        tools.extend(lsp::lsp_tools());
    }
    
    // 添加 workflow 工具
    tools.extend(workflow::workflow_tools());
    
    // 生成描述
    ...
}
```

---

### Phase 4: 系统提示词动态注入

**文件**: `core/src/prompt.rs`

```rust
// 新增 LSP 提示词常量
const SYSTEM_PROMPT_LSP: &str = r#"【LSP 智能感知】

项目已启用 LSP 服务，提供实时代码智能：
- lsp_goto_definition: 跳转到定义（实时、精确）
- lsp_find_references: 查找所有引用（上下文感知）
- lsp_get_diagnostics: 获取语法/类型错误（实时诊断）
- lsp_hover: 查看类型信���和文档（悬停提示）

【工具优先级 - 必须遵守】
1. LSP（实时、准确、上下文感知）- 优先用于精确查找
2. CodeGraph（静态索引、快速）- 优先用于批量搜索
3. grep/search（文本搜索）- 仅用于非代码内容

【常见场景对照】
| 用户请求 | 最佳工具 | 原因 |
|----------|----------|------|
| "跳转到定义" | lsp_goto_definition | LSP 实时解析，精确定位 |
| "查找所有引用" | lsp_find_references | LSP 知道上下文，排除无关 |
| "有什么错误" | lsp_get_diagnostics | LSP 实时诊断，比 grep 准确 |
| "查看类型" | lsp_hover | LSP 提供类型签名和文档 |

【使用条件】
- LSP 工具仅在对应语言的 LSP 服务器运行时可用
- 如果工具不可用，降级使用 CodeGraph 或 grep"#;

const SYSTEM_PROMPT_LSP_SERVERS: &str = r#"【当前可用的 LSP 服务器】

以下 LSP 服务器正在运行并提供智能分析：
{servers_info}

每种语言服务器提供的能力：
- rust-analyzer: Rust 类型推断、宏展开、重构
- typescript-language-server: TypeScript/JavaScript 类型检查
- pylsp: Python 语法检查、自动补全"#;

// 在 build_system_prompt_with_workflows() 中添加动态注入
pub fn build_system_prompt_with_workflows(...) -> String {
    let mut parts = vec![static_prompt, tools_prompt];
    
    // 动态注入 CodeGraph
    if let Some(path) = project_path
        && crate::tools::codegraph::should_inject_codegraph_tools(path) {
        parts.push(SYSTEM_PROMPT_CODEGRAPH_PRACTICE.to_string());
        parts.push(SYSTEM_PROMPT_CODEGRAPH.to_string());
    }
    
    // 动态注入 LSP（新增）
    if crate::tools::lsp::should_inject_lsp_tools() {
        parts.push(SYSTEM_PROMPT_LSP.to_string());
        
        // 动态生成服务器信息
        let servers = crate::lsp::get_active_lsp_servers();
        let servers_info = servers.iter()
            .map(|s| format!("- {}: {} ({})", s.name, s.language, s.status))
            .collect::<Vec<_>>()
            .join("\n");
        
        let servers_prompt = SYSTEM_PROMPT_LSP_SERVERS.replace("{servers_info}", &servers_info);
        parts.push(servers_prompt);
    }
    
    ...
}
```

---

## 🔄 动态注入时机

### LSP 服务器启动流程

```rust
// cli/src/helpers.rs: prepare_lsp_servers()
// 会话启动时：
1. 读取配置文件中的 lsp_servers
2. 启动配置的 LSP 服务器
3. 发送 LspServerAdded 事件

// TUI 监听事件并更新显示
// AI 系统在构建提示词时检查活跃服务器
```

### 系统提示词构建流程

```
用户发起对话
  ↓
build_system_prompt_with_workflows()
  ↓
检查 should_inject_lsp_tools()
  ↓
如果有活跃服务器：
  ├─ 添加 LSP 工具到工具列表
  ├─ 注入 SYSTEM_PROMPT_LSP
  └─ 注入服务器信息
  ↓
发送完整提示词给 AI
```

---

## 📊 优势对比

### 动态注入 vs 静态硬编码

| 特性 | 动态注入 ✅ | 静态硬编码 ❌ |
|------|----------|------------|
| **Token 节省** | 仅在需要时注入 | 始终占用 token |
| **准确性** | 反映实时状态 | 可能与实际不符 |
| **灵活性** | 支持多语言动态切换 | 固定描述 |
| **维护性** | 单点更新 | 需多处修改 |

### Token 统计（估算）

| 场景 | 动态注入 | 静态硬编码 |
|------|---------|-----------|
| 无 LSP | 0 token | ~500 token |
| 1 个 LSP | ~300 token | ~500 token |
| 3 个 LSP | ~400 token | ~500 token |

**节省**: 无 LSP 时节省 ~500 token（约 10%）

---

## 🧪 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_should_inject_lsp_tools_no_manager() {
        // 无 LSP manager 时不应注入
        assert!(!should_inject_lsp_tools());
    }
    
    #[test]
    fn test_should_inject_lsp_tools_with_servers() {
        // 有活跃服务器时应注入
        let manager = MockLspManager::new();
        manager.add_server("rust-analyzer");
        assert!(should_inject_lsp_tools());
    }
    
    #[test]
    fn test_generate_tools_prompt_with_lsp() {
        let prompt = generate_tools_prompt_with_path(None);
        if should_inject_lsp_tools() {
            assert!(prompt.contains("lsp_goto_definition"));
            assert!(prompt.contains("lsp_find_references"));
        }
    }
    
    #[test]
    fn test_system_prompt_includes_lsp() {
        let prompt = build_system_prompt_with_workflows(...);
        if should_inject_lsp_tools() {
            assert!(prompt.contains("【LSP 智能感知】"));
            assert!(prompt.contains("当前可用的 LSP 服务器"));
        }
    }
}
```

---

## 📅 实现计划

### Week 1: 基础工具实现
- 创建 `core/src/tools/lsp.rs`
- 实现 4 个 LSP 工具
- 添加 `should_inject_lsp_tools()` 函数

### Week 2: 系统提示词集成
- 添加 `SYSTEM_PROMPT_LSP` 常量
- 实现动态注入逻辑
- 测试提示词生成

### Week 3: 测试和优化
- 编写单元测试
- 集成测试
- Token 使用优化

---

## 🎯 最终效果

### AI 感知示例

**无 LSP 时**:
```
可用工具：
- code_search: [优先] 搜索代码符号
- grep: 搜索文本内容
...

【CodeGraph 规则】
（如果已初始化）
```

**有 LSP 时**:
```
可用工具：
- lsp_goto_definition: [优先] 跳转���定义（实时）
- lsp_find_references: [优先] 查找引用
- code_search: [优先] 搜索代码符号
...

【LSP 智能感知】
项目已启用 LSP 服务：rust-analyzer (Rust)

【工具优先级】
1. LSP（实时、准确）
2. CodeGraph（快速）
3. grep（文本）
```

---

## ✅ 总结

LSP 动态注入机制的关键优势：

1. **实时性**: 反映当前 LSP 服务器状态
2. **准确性**: 仅在可用时提示 AI
3. **灵活性**: 支持多语言动态组合
4. **高效性**: 节省不必要的 token

这正是 MatrixCode 智能感知系统的核心设计理念！