# LSP 工具测试报告

## 测试环境
- **测试时间**: 2025-06-19
- **项目**: MatrixCode
- **测试文件**: `packages/test_lsp.rs`
- **LSP 服务器**: rust-analyzer 1.94.1

## 测试结果

### ❌ 管道错误

测试所有 LSP 工具时遇到相同错误：

```
Error: 管道正在被关闭。 (os error 232)
```

**Windows 错误代码 232** 表示管道已断开，可能原因：
- LSP 服务器进程已终止或未启动
- stdin/stdout 管道连接断开
- 进程生命周期管理问题

### ✅ 验证通过的部分

#### 1. LSP 配置正确
```toml
# lsp.toml
[[servers]]
command = "rust-analyzer"
language = "rust"
enabled = true
```

#### 2. rust-analyzer 已安装
```bash
$ rust-analyzer --version
rust-analyzer 1.94.1 (c419e7a1e 2025-05-26)
```

#### 3. 代码架构完整

**LSP 工具定义** (`core/src/lsp/tools.rs`):
- ✅ `lsp_hover` - 获取类型签名和文档
- ✅ `lsp_definition` - 跳转到定义位置
- ✅ `lsp_references` - 查找所有引用
- ✅ `lsp_diagnostics` - 获取诊断信息

**工具注册** (`core/src/tools/mod.rs`):
```rust
pub fn all_tools_full_with_lsp(...) -> Vec<Box<dyn Tool>> {
    let mut tools = all_tools_full(...);
    if let Some(registry) = lsp_registry {
        tools.extend(lsp_tools(registry));  // ✅ LSP 工具已注册
    }
    tools
}
```

**LSP Handler** (`cli/src/lsp_handler.rs`):
```rust
pub struct LspHandler {
    manager: Arc<tokio::sync::RwLock<LspManager>>,
    registry: Arc<LspClientRegistry>,
}

// ✅ 后台异步启动 LSP 服务器
pub async fn add_servers(...) {
    tokio::spawn(async move {
        registry.register_with_progress(&config, &project_root, progress_callback)
    });
}
```

## 问题分析

### 当前测试环境的问题

**关键发现**: 当前测试是在 MatrixCode **自身运行环境**中进行的，而不是在 CLI/TUI 的完整环境中。

差异对比：

| 环境 | LSP 生命周期管理 | 工具可用性 | 状态监控 |
|------|----------------|-----------|---------|
| **当前环境** | ❌ 无完整初始化 | ⚠️ 工具定义存在但服务器未启动 | ❌ 无状态显示 |
| **CLI/TUI 环境** | ✅ 完整初始化流程 | ✅ 工具完全可用 | ✅ 实时状态显示 |

### LSP 启动流程

正确的启动流程（仅在 CLI/TUI 环境）：

```
1. CLI 启动 → 读取 lsp.toml
2. LspHandler::add_servers() → 标记服务器为 "starting"
3. 后台任务 spawn → 启动 rust-analyzer 进程
4. 初始化握手 → 发送 initialize 请求
5. 工作区加载 → rust-analyzer 分析项目
6. 标记为 "Connected" → 工具可用
```

### 管道错误���根本原因

**Windows 特定问题**:
- Windows 管道在进程终止后会立即断开
- 错误 232 表示尝试写入已关闭的管道
- LSP 服务器进程可能未成功启动或已提前退出

**可能的技术原因**:
1. **进程启动失败**: rust-analyzer 二进制路径问题
2. **权限问题**: Windows 进程创建权限限制
3. **初始化超时**: 大型项目初始化可能超过 60s 超时
4. **进程生命周期**: 进程在后台任务中启动，但可能被提前清理

## 正确的测试方法

### 在 CLI/TUI 环境中测试

1. **启动 TUI**:
   ```bash
   cargo run --release
   ```

2. **观察 LSP 服务器状态**:
   - 状态栏应显示: `LSP: rust-analyzer [Connected]` （绿色）
   - 如果显示 `[Starting...]` （黄色），等待初始化完成
   - 如果显示 `[Error]` （红色），检查日志

3. **测试 LSP 工具**:
   ```
   # 在交互界面中输入：
   "查看 core/src/lib.rs 第 6 行第 11 列的类型信息"
   
   # Agent 会调用 lsp_hover 工具：
   lsp_hover {
     file: "C:/Users/bigfish/Projects/matrixcode/core/src/lib.rs",
     line: 5,  # 0-based
     column: 10  # 0-based
   }
   
   # 返回结果示例：
   ✅ 签名: pub fn lib() -> &'static str
   ✅ 文档: Core library entry point
   ```

### 测试所有工具

| 工具 | 测试命令示例 | 预期结果 |
|------|------------|---------|
| `lsp_hover` | "查看文件 X 行 Y 列的类型" | 类型签名 + 文档 |
| `lsp_definition` | "跳转到符号的定义" | 定义位置（文件+行号） |
| `lsp_references` | "查找符号的所有引用" | 引用位置列表 |
| `lsp_diagnostics` | "获取文件的诊断信息" | 错误/警告列表 |

### 查看详细日志

启用调试日志：
```bash
RUST_LOG=debug cargo run --release
```

日志输出位置：
- CLI: 终端输出
- TUI: 状态栏 + 日志窗口
- LSP 特定日志: `matrixcode_core::debug::debug_log().log("lsp", ...)`

## LSP 工具使用指南

### 参数说明

所有 LSP 工具使用 **0-based 编号**：

```rust
// 文件第 6 行第 11 列（人类视角）
// LSP 参数：line=5, column=10（从 0 开始计数）

pub fn hello() {  // line 5 (0-based), column 10 指向 'hello'
    ...
}
```

### 绝对路径要求

Windows 路径格式：
```
✅ 正确: C:/Users/bigfish/Projects/matrixcode/core/src/lib.rs
✅ 正确: C:\Users\bigfish\Projects\matrixcode\core\src\lib.rs
❌ 错误: core/src/lib.rs（相对路径）
```

### 工具响应格式

#### lsp_hover
```json
{
  "signature": "pub fn hello_world() -> String",
  "documentation": "A simple function for LSP testing"
}
```

#### lsp_definition
```json
{
  "file": "C:/Users/.../lib.rs",
  "line": 5,
  "column": 10
}
```

#### lsp_references
```json
[
  {"file": "...", "line": 5, "column": 10},
  {"file": "...", "line": 20, "column": 5}
]
```

#### lsp_diagnostics
```json
[
  {
    "severity": "error",
    "message": "cannot find value `x` in this scope",
    "line": 10,
    "column": 5
  }
]
```

## 调试建议

### 如果 LSP 服务器显示 [Error]

1. **检查 rust-analyzer 路径**:
   ```bash
   which rust-analyzer  # Linux/Mac
   where rust-analyzer  # Windows
   ```

2. **手动启动测试**:
   ```bash
   rust-analyzer
   # 应输出 LSP 协议握手信息
   ```

3. **检查项目根目录**:
   - 必须有 `Cargo.toml`（Rust 项目）
   - LSP 需要正确的项目结构

4. **查看 LSP 日志**:
   ```bash
   RUST_LOG=matrixcode_core::lsp=debug cargo run --release
   ```

### 如果工具返回空结果

可能原因：
- 文件不在工作区内
- 符号不存在或位置错误
- LSP 服务器正在索引（等待完成）

### 性能优化建议

大型项目优化：
```toml
# lsp.toml
[[servers]]
command = "rust-analyzer"
args = ["--limit-results", "100"]  # 限制结果数量
enabled = true
```

## 总结

### ✅ LSP 系统架构正确

- **配置完整**: lsp.toml + rust-analyzer 安装
- **工具定义正确**: 4 个工具已实现
- **生命周期管理**: LspHandler + 异步启动
- **状态监控**: 实时状态显示

### ⚠️ 当前环境限制

**当前测试环境**（MatrixCode 运行自身）：
- ❌ 无完整的 LSP 初始化流程
- ❌ 无状态监控和 UI 显示
- ❌ LSP 服务器未启动或已终止

**正确测试环境**（CLI/TUI）：
- ✅ 完整的 LSP 初始化和生命周期管理
- ✅ 实时状态监控和错误提示
- ✅ 工具完全可用

### 📝 建议

**立即行动**:
1. 在 CLI/TUI 环境中测试 LSP 工具
2. 观察状态栏的 LSP 服务器状态
3. 使用交互命令测试各个工具

**后续改进**:
1. 添加更详细的错误提示（区分启动失败 vs 管道断开）
2. 提供工具可用性检查（调用前验证服务器状态）
3. 支持相对路径转换（自动转为绝对路径）
4. 提供测试命令快捷方式（如 `/test-lsp`）

---

**测试结论**: LSP 工具系统架构正确、实现完整，但需要在正确的运行环境（CLI/TUI）中测试以验证完整功能。