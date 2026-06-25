# MatrixCode 代码门禁系统

## 概述

MatrixCode 现在拥有**双层代码门禁**系统：

1. **运行时代码门禁**（新）- AI 写入代码时实时检查
2. **Git 预提交门禁**（可选）- 提交前本地检查

---

## 1. 运行时代码门禁 ⭐

### 功能
`WriteTool` 现在会在写入代码文件前进行**语法验证**：

| 语言 | 验证方式 | 拦截场景 |
|-----|---------|---------|
| **Rust** | `rustfmt --check` | 语法错误、格式问题 |
| **TypeScript/JavaScript** | 括号匹配检查 | 花括号/括号不匹配 |
| **Python** | `python -m py_compile` | 语法错误 |

### 工作流程

```
AI 调用 Write Tool
       ↓
检查文件类型 (.rs/.ts/.js/.py)
       ↓
运行语法验证
       ↓
┌─────────────────┐
│ 验证通过?        │
└─────────────────┘
       ↓
   是 / 否
   ↓     ↓
写入文件  返回错误给 AI
          "🚫 代码门禁拦截：..."
          "请修正后再写入"
```

### 示例

**AI 尝试写入错误代码**：
```rust
// AI 生成的错误代码
fn main() {
    println!("Hello"
    // 缺少右括号
}
```

**代码门禁拦截**：
```
🚫 代码门禁拦截：Rust 语法错误
error: expected `}`, found `EOF`

请修正错误后再写入。
```

**AI 收到错误后修正**：
```rust
// AI 修正后的代码
fn main() {
    println!("Hello");
}
```

**代码门禁通过，写入成功** ✅

---

## 2. Git 预提交门禁

### 安装

```bash
# 启用 Git hooks
chmod +x .githooks/pre-commit
git config core.hooksPath .githooks
```

### 检查项

| 检查项 | 说明 |
|-------|-----|
| Rust 格式化 | `cargo fmt --all -- --check` |
| Clippy 检查 | `cargo clippy --all-targets --all-features -- -D warnings` |
| 测试 | `cargo test` |
| 版本一致性 | 检查所有 Cargo.toml 版本一致 |
| CHANGELOG | 版本升级时提醒更新 |
| 大文件 | 阻止 >1MB 的文件 |
| Session 文件 | 阻止提交 `.matrix/sessions/` |
| TODO 统计 | 提醒 TODO/FIXME 数量 |

---

## 3. CI/CD 门禁

已配置在 `.github/workflows/ci.yml`：

- **多平台测试** (Ubuntu, macOS, Windows)
- **格式化检查** (`cargo fmt`)
- **Lint 检查** (`cargo clippy`)
- **VSCode 扩展构建**

---

## 4. 配置方式

### 运行时策略（Agent 配置）

```rust
// 在 AgentConfig 中配置
AgentConfig {
    verify_strategy: VerificationStrategy::Pre, // 写入前验证
    project_path: Some("/path/to/project".into()),
    ..Default::default()
}
```

策略选项：
- `None` - 不验证
- `Post` - 写入后验证（默认）
- `Pre` - 写入前验证，拦截错误
- `PreQuick` - 快速预检查 + 完整后检查

### 环境变量

```bash
# 设置验证策略
export VERIFY_STRATEGY=pre

# 或在 .matrix/config.json 中
{
  "verify_strategy": "pre"
}
```

---

## 5. 文件变更

### 新增文件
- `packages/core/src/tools/code_quality_hook.rs` - 代码质量钩子实现
- `packages/core/src/tools/tool_hooks.rs` - 工具钩子框架
- `.githooks/pre-commit` - Git 预提交脚本

### 修改文件
- `packages/core/src/tools/write.rs` - 集成运行时验证
- `packages/core/src/tools/mod.rs` - 导出新模块
- `packages/core/Cargo.toml` - 添加 tempfile 依赖

---

## 6. 验证测试

### 测试运行时门禁

```bash
# 启动 MatrixCode
matrixcode

# 让 AI 写入错误代码
> 写一个缺少括号的 Rust 函数

# 观察：AI 尝试写入后收到错误，修正后再写入
```

### 测试 Git 门禁

```bash
# 尝试提交未格式化的代码
git add .
git commit -m "test"

# 观察：pre-commit hook 拦截，提示格式化
```

---

## 7. 效果预期

| 问题类型 | 修复前 | 修复后 |
|---------|-------|-------|
| 语法错误写入 | 直接写入，后续编译失败 | **写入前拦截** |
| 格式不一致 | 代码风格混乱 | **自动检查** |
| 版本不一致 | 多个 Cargo.toml 版本不同 | **提交前检查** |
| Session 文件提交 | 可能误提交大文件 | **自动阻止** |

---

## 8. 关闭门禁（不推荐）

```bash
# 运行时关闭
export VERIFY_STRATEGY=none

# Git hook 跳过
git commit --no-verify -m "message"
```

---

## 总结

✅ **运行时代码门禁** - 防止 AI 写入错误代码  
✅ **Git 预提交门禁** - 保证提交代码质量  
✅ **CI/CD 门禁** - 多平台自动化测试  

三层防护，确保代码质量！🛡️