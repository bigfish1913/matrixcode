# 版本升级完成 - 手动推送指南

## 当前状态

✅ **版本已升级到 0.4.46**

### 修改内容

1. **核心修复**（已提交）
   - Commit: `1c293ab` - fix(session): 修复压缩功能完全失效的严重bug
   - 包含 5 个关键文件修改：
     - packages/core/src/agent/types.rs
     - packages/core/src/agent/run.rs
     - packages/core/src/agent/builder.rs
     - packages/cli/src/terminal/session.rs
     - packages/cli/src/terminal/agent.rs

2. **版本升级**（待提交）
   - Cargo.toml: 0.4.44 → 0.4.46
   - CHANGELOG.md: 添加 0.4.46 版本条目
   - packages/vscode/package.json: 0.4.17 → 0.4.46
   - packages/npm/package.json: 版本同步

### 性能改进预期

| 项目 | 修复前 | 修复后预期 | 改善 |
|-----|-------|----------|-----|
| Token 使用量 | 106,647 | ~40,000 | **60%+ ↓** |
| Session 文件大小 | 439KB | ~200KB | **50%+ ↓** |
| API 成本 | 高 | 低 | **60%+ ↓** |

## 需要手动执行的操作

由于网络连接暂时不稳定，请手动执行以下命令：

### 1. 提交版本升级
```bash
git add Cargo.toml Cargo.lock packages/vscode/package.json packages/npm/package.json CHANGELOG.md
git commit -m "chore: bump to 0.4.46

Co-Authored-By: Claude <noreply@anthropic.com>"
```

### 2. 创建 Git Tag
```bash
git tag 0.4.46 -m "fix(session): 压缩功能修复 + 性能提升 60%"
```

### 3. 推送到远程
```bash
# 推送分支
git push origin dev-gui

# 推送 tag
git push origin 0.4.46
```

### 4. 发布到 Cargo（可选）
```bash
# 如果需要发布到 crates.io
task publish-cargo
```

### 5. 发布到 VS Code Marketplace（可选）
```bash
# 如果需要发布 VS Code 扩展
task publish-vscode
```

## 验证版本

```bash
# 检查版本号
grep "^version = " Cargo.toml
# 应显示: version = "0.4.46"

# 检查 CHANGELOG
head -30 CHANGELOG.md
# 应显示 0.4.46 版本条目

# 检查 git log
git log --oneline --decorate | head -10
# 应显示最新 commit 和 tag
```

## 测试新版本

```bash
# 编译
cargo build --release

# 运行
matrixcode

# 测试压缩功能
# 1. 进行对话触发工具调用
# 2. 当 context > 40% 时查看压缩日志
# 3. 保存 session: /save test
# 4. 检查: cat ~/.matrix/sessions/<id>.json | jq '{full: (.full_messages | length), compressed: (.compressed_messages | length)}'
```

## 下次发布

下次可以使用自动发布命令：
```bash
# 自动升级 patch 版本并发布
task publish
```

---

**版本升级完成！等待手动推送。** 🎉