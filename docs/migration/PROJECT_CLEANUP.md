# MatrixCode 项目整理指南

## 一、当前项目结构

```
packages/cli/
├── Cargo.toml              # Workspace配置
├── Cargo.lock              # 依赖锁定
├── README.md               # 项目说明
├── .env.example            # 环境变量示例
│
├── crates/                 # ✅ 核心代码
│   ├── matrixcode-core/    # Agent核心
│   ├── matrixcode-tui/     # Terminal UI
│   └── matrixcode-cli/     # CLI入口
│
├── tests/                  # ⚠️ 需修复（引用旧模块）
│   └── test_*.rs           # 14个测试文件
│
├── docs/                   # ✅ 文档
│   ├── FEATURE_COMPLETE.md
│   ├── FINAL_REPORT.md
│   ├── MIGRATION_ANALYSIS.md
│   └── PROJECT_COMPLETE.md
│
├── npm/                    # ✅ npm发布包
│   ├── package.json
│   ├── install.js
│   └── README.md
│
├── skills/                 # ✅ 技能目录
│   └── *.md
│
├── _src_old/               # ⚠️ 可删除（676KB）
│   └── *.rs                # 18个旧文件
│
└── target/                 # 编译输出
    └── release/
        └── matrixcode.exe  # ✅ 0.3.0
```

---

## 二、清理命令

### 删除旧代码备份
```bash
rm -rf _src_old/
# 节省 676KB
```

### 删除无用文件
```bash
# 删除备份文件
rm -f crates/matrixcode-core/src/agent_full.rs.bak
rm -f crates/matrixcode-core/src/agent_simple.rs.bak

# 清理编译缓存（可选）
cargo clean
```

### 整理测试目录
```bash
# 选项1: 移动测试到 core crate
mv tests/ crates/matrixcode-core/tests/

# 选项2: 修复测试引用（改 matrixcode:: 为 matrixcode_core::）
sed -i 's/use matrixcode::/use matrixcode_core::/g' tests/*.rs

# 选项3: 暂时删除旧测试（后续重写）
rm -rf tests/
```

---

## 三、测试命令

### 单元测试
```bash
# 运行所有测试
cargo test

# 运行特定crate测试
cargo test -p matrixcode-core
cargo test -p matrixcode-tui
cargo test -p matrixcode-cli

# 运行单个测试
cargo test test_event

# 显示测试输出
cargo test -- --nocapture
```

### 集成测试（需修复）
```bash
# 修复tests引用后
cargo test --test test_tools_mod
```

### 手动测试
```bash
# 测试CLI
./target/release/matrixcode --version
./target/release/matrixcode --help
./target/release/matrixcode chat --message "Hello"

# 测试daemon模式
echo '{"type":"chat","content":"test"}' | ./target/release/matrixcode --mode daemon

# 测试VSCode插��
cd packages/vscode
npm run compile
# F5 在VSCode中调试
```

---

## 四、发布命令

### 1. Cargo 发布（crates.io）

```bash
# 检查package
cargo package -p matrixcode-core
cargo package -p matrixcode-tui
cargo package -p matrixcode

# 发布到crates.io（需登录）
cargo publish -p matrixcode-core
cargo publish -p matrixcode-tui
cargo publish -p matrixcode

# 安装到本地
cargo install --path .
```

### 2. npm 发布

```bash
cd npm

# 更新版本
npm version patch  # 或 minor / major

# 发布到npm
npm publish

# 本地测试安装
npm pack
npm install -g matrixcode-0.3.0.tgz
```

### 3. GitHub Release

```bash
# 构建二进制
cargo build --release

# 创建发布包
mkdir -p release
cp target/release/matrixcode.exe release/
cp README.md release/
cp LICENSE release/

# 打包
cd release
zip matrixcode-windows-x64.zip matrixcode.exe README.md LICENSE

# 上传到GitHub Release
# 在 GitHub 网页创建新 Release，上传 zip 文件
```

---

## 五、CI配置建议

创建 `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test

  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: matrixcode-${{ matrix.os }}
          path: target/release/matrixcode*
```

---

## 六、完整清理脚本

```bash
#!/bin/bash
# cleanup.sh

echo "清理旧代码备份..."
rm -rf _src_old/

echo "清理临时文件..."
rm -f crates/matrixcode-core/src/*.bak
rm -f crates/matrixcode-core/src/agent_full.rs.bak

echo "清理编译缓存..."
cargo clean

echo "整理文档..."
mkdir -p docs/archive
mv docs/CLI_SPLIT_*.md docs/archive/ 2>/dev/null || true

echo "清理完成！"
echo ""
echo "剩余结构:"
ls -la
```

---

## 七、验证清单

```bash
# 编译检查
cargo build --release && echo "✅ 编译成功"

# 测试检查
cargo test && echo "✅ 测试通过"

# CLI功能检查
./target/release/matrixcode --version && echo "✅ 版本正常"
./target/release/matrixcode --help && echo "✅ 帮助正常"
echo '{"type":"chat","content":"test"}' | ./target/release/matrixcode --mode daemon && echo "✅ Daemon正常"

# VSCode插件检查
cd packages/vscode && npm run compile && echo "✅ 插件编译成功"

# 发布准备检查
cargo package --list && echo "✅ Package准备完成"
```

---

## 八、下一步建议

| 任务 | 优先级 | 说明 |
|------|--------|------|
| 删除 _src_old/ | P1 | 节省676KB |
| 修复tests引用 | P2 | 改 matrixcode:: 为 matrixcode_core:: |
| 添加CI配置 | P2 | GitHub Actions |
| 发布到crates.io | P3 | cargo publish |
| 发布到npm | P3 | npm publish |
| 创建GitHub Release | P3 | 打包二进制 |

---

## 九、文件大小统计

```
当前总大小: ~50MB (含target)
清理后: ~5MB (不含target)

_src_old/: 676KB
target/: ~45MB (cargo clean可清理)
docs/: ~20KB
源代码: ~3MB
```

---

## 十、项目状态

✅ **编译**: 通过
✅ **测试**: 单元测试通过
✅ **CLI**: 功能正常
✅ **Daemon**: JSON输出正常
✅ **VSCode**: 编译成功
⚠️ **集成测试**: 需修复引用
⚠️ **CI**: 未配置
⚠️ **发布**: 待执行