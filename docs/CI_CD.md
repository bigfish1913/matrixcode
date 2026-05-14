# GitHub Actions CI/CD 使用指南

本项目已配置 GitHub Actions 自动化 CI/CD 流程。

## 工作流程

### 1. 持续集成 (CI)

**触发条件：**
- 推送到 `main`、`master` 或 `develop` 分支
- 创建 Pull Request 到上述分支

**执行任务：**
- **多平台测试**：在 Linux、macOS、Windows 上运行测试
- **代码检查**：
  - `cargo fmt` 格式检查
  - `cargo clippy` lint 检查

**查看结果：**
在 GitHub 仓库的 Actions 标签页可以查看运行状态。

### 2. 自动发布 (Release)

**触发条件：**
- 创建以 `v` 开头的标签（如 `v1.0.0`）

**构建目标：**
- `x86_64-unknown-linux-gnu` (Linux x86_64)
- `aarch64-unknown-linux-gnu` (Linux ARM64)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-pc-windows-msvc` (Windows x86_64)
- `aarch64-pc-windows-msvc` (Windows ARM64)

**发布流程：**
1. 编译各平台二进制文件
2. 打包为 `.tar.gz` (Unix) 或 `.zip` (Windows)
3. 创建 GitHub Release
4. 上传编译产物

## 使用方法

### 发布新版本

```bash
# 1. 更新版本号（可选）
# 编辑 Cargo.toml 中的 version 字段

# 2. 提交更改
git add .
git commit -m "chore: bump version to x.x.x"

# 3. 创建标签
git tag -a v1.0.0 -m "Release v1.0.0"

# 4. 推送标签到 GitHub
git push origin v1.0.0
```

推送标签后，GitHub Actions 会自动开始构建并创建 Release。

### 查看构建状态

- 在仓库首页会显示最近的工作流状态徽章
- 点击 "Actions" 标签查看详细的构建日志

### 下载发布版本

用户可以从 GitHub Releases 页面下载对应平台的二进制文件：
```
https://github.com/YOUR_USERNAME/matrixcode/releases
```

## 本地测试 CI

在推送前可以本地运行类似的检查：

```bash
# 运行测试
cargo test

# 格式检查
cargo fmt --all -- --check

# Lint 检查
cargo clippy --all-targets --all-features -- -D warnings

# 构建发布版本
cargo build --release
```

## 配置说明

### 环境变量

CI 流程中使用的主要环境变量：
- `CARGO_TERM_COLOR: always` - 彩色输出
- `RUST_BACKTRACE: 1` - 启用错误回溯

### 缓存策略

为了加速构建，CI 会缓存：
- `~/.cargo/registry` - Cargo 注册表
- `~/.cargo/git` - Git 依赖
- `target` - 编译产物

缓存键基于 `Cargo.lock` 文件的哈希值，依赖变更时会自动更新缓存。

## 自定义配置

### 修改分支

编辑 `.github/workflows/ci.yml` 中的 `branches` 列表：
```yaml
on:
  push:
    branches: [ main, master, develop, your-branch ]
```

### 添加新的构建目标

在 `.github/workflows/release.yml` 的 `matrix.include` 中添加：
```yaml
- target: your-target-triple
  os: runner-os
  archive: tar.gz  # 或 zip
```

### 跳过 CI

在提交消息中添加 `[skip ci]` 或 `[ci skip]` 可以跳过 CI：
```bash
git commit -m "docs: update readme [skip ci]"
```

## 故障排查

### 构建失败

1. 查看 Actions 日志中的错误信息
2. 本地运行 `cargo test` 和 `cargo clippy` 检查
3. 确保依赖完整，本地 `Cargo.lock` 与远程一致

### 发布失败

- 确保 tag 格式正确（以 `v` 开头）
- 检查 GitHub Token 权限
- 查看是否有编译错误

## 相关链接

- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Rust Actions 示例](https://github.com/actions-rs/example)