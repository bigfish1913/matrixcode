# MatrixCode npm Package

这个 npm 包是 [MatrixCode](https://github.com/bigfish1913/matrixcode) 的安装包装器。

## 安装

```bash
npm install -g matrixcode
```

安装脚本会自动：
1. 检查是否已安装 `matrixcode`
2. 如果有 Rust/Cargo，使用 `cargo install matrixcode`
3. 否则从 GitHub Releases 下载预编译二进制

## 手动安装

如果自动安装失败，你可以：

```bash
# 使用 Cargo
cargo install matrixcode

# 或从 GitHub Releases 下载
# https://github.com/bigfish1913/matrixcode/releases
```

## 发布说明

发布新版本时：

1. 更新版本号：
   - `Cargo.toml` 中的 `version`
   - `npm/package.json` 中的 `version`

2. 发布到 crates.io：
   ```bash
   cargo login
   cargo publish
   ```

3. 发布到 npm：
   ```bash
   cd npm
   npm login
   npm publish
   ```

4. 创建 Git tag 并推送（触发 GitHub Release）：
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

## 支持的平台

- macOS (x64, Apple Silicon)
- Linux (x64, arm64)
- Windows (x64)