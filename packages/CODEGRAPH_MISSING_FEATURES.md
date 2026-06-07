# CodeGraph 遗漏功能分析报告

## 问题澄清

用户反馈：**CodeGraph 远不止已有的工具，还有其他功能未添加**

经过重新检查，发现：

1. **VerifyTool** - 已通过 Hook 系统集成（`code_quality_hook.rs`），不需要作为独立工具
2. **CodeGraph manager.rs** - 有 `files()` 方法**未封装成工具**

---

## 一、已封装的 CodeGraph 工具（5 个）

| 工具名 | Manager 方法 | 功能 |
|--------|-------------|------|
| `code_search` | `search()` | 搜索符号（优先工具） |
| `code_callers` | `callers()` | 查找调用者（优先工具） |
| `code_callees` | `callees()` | 查找被调用者（优先工具） |
| `code_status` | `status()` | 索引状态 |
| `code_sync` | `sync()` | 手动同步 |

---

## 二、🔴 遗漏的 CodeGraph 功能

### `files()` 方法 - 未封装！

**文件位置**: `core/src/tools/codegraph/manager.rs:322-358`

**功能**: 查询索引中的文件列表，支持按语言过滤

**返回数据**: `FileInfo`
```rust
pub struct FileInfo {
    pub path: String,         // 文件路径
    pub language: String,     // 语言类型
    pub size: u64,            // 文件大小
    pub modified: u64,        // 修改时间
    pub node_count: Option<u32>, // 符号数量
}
```

**���用场景**:
- 查看项目中哪些文件被索引
- 按语言过滤（如只看 Rust 文件）
- 了解每个文件的符号密度
- 快速浏览项目结构（比 glob 更智能）

**对比现有工具**:
- `glob` - 按文件名模式查找
- `code_files` - 按语言过滤，返回符号数量（更语义化）

---

## 三、建议新增工具：code_files

### 3.1 工具定义

```rust
pub struct CodeGraphFilesTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphFilesTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_files".to_string(),
            description: "查询索引中的文件列表。返回文件路径、语言类型、符号数量。支持按语言过滤。比 glob 更智能，能显示每个文件的符号密度。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "按语言过滤（如 'rust', 'typescript', 'python'），不填则返回所有文件"
                    }
                }
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let language = args["language"].as_str();
        let files = self.manager.files(language)?;
        
        Ok(serde_json::to_string(&json!({
            "files": files,
            "total_count": files.len(),
            "filter": language
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}
```

### 3.2 使用示例

**场景 1：查看所有被索引的文件**
```
code_files()
→ 返回：所有文件，包含符号数量
```

**场景 2：只看 Rust 文件**
```
code_files(language: "rust")
→ 返回：所有 .rs 文件及其符号数量
```

**场景 3：只看 TypeScript 文件**
```
code_files(language: "typescript")
→ 返回：所有 .ts/.tsx 文件及其符号数量
```

---

## 四、实施步骤

### 4.1 在 tools.rs 中添加

**文件**: `core/src/tools/codegraph/tools.rs`

```rust
/// Tool for querying indexed files.
pub struct CodeGraphFilesTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphFilesTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_files".to_string(),
            description: "查询索引中的文件列表。返回文件路径、语言、符号数量。支持按语言过滤。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "按语言过滤（如 'rust', 'typescript'），不填返回所有"
                    }
                }
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let language = args.get("language").and_then(|v| v.as_str());
        let files = self.manager.files(language)?;
        Ok(serde_json::to_string(&json!({
            "files": files,
            "total_count": files.len()
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}
```

### 4.2 在 codegraph_tools() 中注册

**文件**: `core/src/tools/codegraph/tools.rs:264-273`

```rust
pub fn codegraph_tools(project_path: &Path) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CodeGraphSearchTool::new(project_path)),
        Box::new(CodeGraphCallersTool::new(project_path)),
        Box::new(CodeGraphCalleesTool::new(project_path)),
        Box::new(CodeGraphStatusTool::new(project_path)),
        Box::new(CodeGraphSyncTool::new(project_path)),
        Box::new(CodeGraphFilesTool::new(project_path)), // ← 新增
    ]
}
```

### 4.3 更新导出

**文件**: `core/src/tools/codegraph/mod.rs:22-26`

```rust
pub use self::tools::{
    CodeGraphCalleesTool, CodeGraphCallersTool, CodeGraphFilesTool, // ← 新增
    CodeGraphSearchTool, CodeGraphStatusTool, CodeGraphSyncTool,
    codegraph_tools, codegraph_tools_if_installed,
    codegraph_tools_with_auto_detect, should_inject_codegraph_tools,
};
```

---

## 五、其他 Manager 方法分析

| 方法 | 是否需要封装 | 原因 |
|------|-------------|------|
| `init()` | ❌ 不需要 | 初始化命令，不是 Agent 工具（用户手动执行） |
| `reinit()` | ❌ 不需要 | 重建索引，不是 Agent 工具（用户手动执行） |
| `ensure_initialized()` | ❌ 不需要 | 内部方法，Watcher 使用 |
| `files()` | ✅ **需要** | Agent 可用：查询文件列表 |

---

## 六、验证测试

### 单元测试

```rust
#[test]
fn test_code_files_tool() {
    let tool = CodeGraphFilesTool::new(&PathBuf::from("."));
    let def = tool.definition();
    
    assert_eq!(def.name, "code_files");
    assert!(def.description.contains("文件列表"));
    assert_eq!(tool.risk_level(), RiskLevel::Safe);
}

#[test]
fn test_code_files_in_codegraph_tools() {
    let tools = codegraph_tools(&PathBuf::from("."));
    let names = tools.iter().map(|t| t.definition().name).collect::<Vec<_>>();
    
    assert!(names.contains(&"code_files".to_string()));
}
```

---

## 七、总结

### 遗漏功能

- ✅ **VerifyTool** - 已通过 Hook 集成（用户正确）
- 🔴 **code_files** - Manager 的 `files()` 方法未封装（真正的遗漏）

### 建议行动

1. **立即添加** `CodeGraphFilesTool`（5 分钟工作量）
2. 注册到 `codegraph_tools()` 函数
3. 更新导出和测试

### 工具完整度

- 已封装：**5 个工具**
- 遗漏：**1 个工具**（files）
- 封装率：**83.3%** → 添加后 **100%**

---

**报告生成**: 2026-06-17  
**真正遗漏**: code_files 工具  
**工作量**: 5 分钟（添加工具定义 + 注册）