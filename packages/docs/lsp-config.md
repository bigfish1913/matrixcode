# LSP (Language Server Protocol) 自动检测

## 自动检测机制

Matrixcode **自动检测**已安装的 LSP 服务器，无需配置。

### 检测流程

启动时自动扫描以下常见 LSP 服务器：

| 语言 | LSP 服务器 | 检测命令 |
|------|-----------|---------|
| Rust | rust-analyzer | `rust-analyzer` |
| TypeScript | typescript-language-server | `typescript-language-server --stdio` |
| Python | pylsp | `pylsp` |
| Python | pyright | `pyright --stdio` |
| Go | gopls | `gopls` |
| C/C++ | clangd | `clangd` |
| Java | jdtls | `jdtls` |

### 安装 LSP 服务器

只需安装对应语言的服务器，Matrixcode 会自动检测并使用：

```bash
# Rust
rustup component add rust-analyzer

# TypeScript/JavaScript
npm install -g typescript-language-server typescript

# Python (任选其一)
pip install python-lsp-server
# 或
pip install pyright

# Go
go install golang.org/x/tools/gopls@latest

# C/C++
# 通常随 LLVM/Clang 安装

# Java
# 通常随 IDE 或单独安装 jdtls
```

### 验证检测

启动 Matrixcode 后：
1. 工具栏显示 `LSP:N`（浅紫红色，N 为已连接数量）
2. 系统消息显示 `🔤 LSP 'rust-analyzer' (rust) 已添加`
3. 日志中可见 `LSP server 'xxx' detected and available`

### 状态指示

| 状态 | 显示 | 说明 |
|------|------|------|
| Connected | LSP:N (绿色) | 正常工作 |
| NotStarted | LSP:0 (灰色) | 未检测到 |
| Error | LSP:N (红色) | 启动失败 |

## 工作原理

```rust
// helpers.rs: prepare_lsp_servers()
// 自动检测逻辑
if is_command_available("rust-analyzer") {
    // 添加到服务器列表
}
```

检测方法：
- Unix: `which command`
- Windows: `where command`

## 零配置设计

**无需任何配置文件**：
- 不需要在 `config.json` 中添加 `lsp_servers`
- 不需要手动指定路径或参数
- 自动使用标准启动参数

## 与 MCP 对比

| 特性 | MCP | LSP |
|------|-----|-----|
| 配置 | 需要 `mcp.toml` | **自动检测** |
| 检测 | 手动配置 | 自动扫描 |
| 工具栏颜色 | Cyan | 根据状态动态变化 |
| 用途 | 外部工具 | 代码智能 |

## 未来功能

框架已就绪，后续可添加：
- 代码补全工具（使用 LSP）
- 跳转定义工具
- 查找引用工具
- 符号重命名工具