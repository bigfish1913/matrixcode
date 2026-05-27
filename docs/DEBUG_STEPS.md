# 调试 MatrixCode 扩展 - 详细步骤

## ⚠️ 重要前提

**必须在 `packages/vscode` 目录下打开 VSCode！**

```bash
cd packages/vscode
code .
```

## 步骤 1: 检查文件是否正确

在终端运行：
```bash
cd packages/vscode
ls -la dist/          # 应该有 extension.js
ls -la resources/     # 应该有 icon.svg
cat package.json | grep "main"  # 应该是 "./dist/extension.js"
```

## 步骤 2: 编译开发版本

```bash
cd packages/vscode
npm run compile-dev
```

应该看到：
```
dist\extension.js      33.4kb
dist\extension.js.map  58.3kb
⚡ Done in 4ms
```

## 步骤 3: 按 F5 启动调试

1. 在 VSCode 中按 `F5`
2. 选择 **"Run Extension (Debug)"**
3. 等待新的 VSCode 窗口打开（扩展开发宿主）

## 步骤 4: 在扩展宿主窗口检查

### 4.1 等待扩展激活

扩展使用 `onStartupFinished` 激活事件，需要等待几秒钟。

### 4.2 查看输出日志 ⭐ 最重要

1. 按 `Ctrl+Shift+U` 打开输出面板
2. 在右侧下拉菜单选择 **"MatrixCode"**
3. 查看日志输出：

应该看到：
```
MatrixCode extension is activating...
Extension path: C:\Users\...\packages\vscode
ConfigManager initialized
MatrixCodeClient initialized
ChatViewProvider registered
Commands registered
CLI availability: false
StatusBar item added
MatrixCode extension activated successfully!
```

### 4.3 查看状态栏

底部状态栏左侧应该有：**🤖 MatrixCode**

点击它会打开聊天视图。

### 4.4 查看活动栏

左侧活动栏（最左侧的图标栏）应该有 MatrixCode 图标。

### 4.5 查看命令列表

1. 按 `Ctrl+Shift+P`
2. 输入 **"MatrixCode"**
3. 应该看到命令列表：
   - MatrixCode: Explain Code
   - MatrixCode: Fix Code
   - MatrixCode: Generate Tests
   - MatrixCode: Refactor Selection
   - MatrixCode: New Session
   - MatrixCode: Open Settings

## 如果看不到扩展

### 检查 1: 确认在正确目录

在调试的 VSCode 中按 `Ctrl+Shift+P`，输入：
```
Developer: Show Running Extensions
```

查看是否有 "MatrixCode" 扩展。

### 检查 2: 手动激活扩展

按 `Ctrl+Shift+P`，输入：
```
MatrixCode: Open Settings
```

这会触发扩展激活。

### 检查 3: 查看扩展开发宿主日志

按 `Ctrl+Shift+U`，选择不同的输出通道：
- "MatrixCode" - 扩展日志
- "Log (Extension Host)" - VSCode 扩展宿主日志

### 检查 4: 重载扩展

在扩展宿主窗口按 `Ctrl+R` 重载。

### 检查 5: 检查编译输出

```bash
cd packages/vscode
ls -la dist/
# 必���有 extension.js (33.4kb)
```

## 快速验证脚本

在 packages/vscode 目录创建 test.sh：

```bash
#!/bin/bash
echo "=== Checking MatrixCode Extension ==="
echo "1. Checking dist directory..."
ls -la dist/

echo "2. Checking resources directory..."
ls -la resources/

echo "3. Checking package.json..."
cat package.json | grep "main"
cat package.json | grep "activationEvents" -A 4

echo "4. Compiling..."
npm run compile-dev

echo "5. Extension ready to debug!"
echo "Press F5 in VSCode to start debugging"
```

运行：
```bash
cd packages/vscode
bash test.sh
```

## 常见问题

### Q1: 按 F5 没反应？
**A**: 确保在 packages/vscode 目录打开 VSCode，不是根目录！

### Q2: 扩展宿主窗口空白？
**A**: 等待几秒让扩展激活（onStartupFinished）

### Q3: 状态栏没有 MatrixCode？
**A**: 按 Ctrl+Shift+U，选择 "MatrixCode" 查看是否有错误日志

### Q4: 活动栏没有图标？
**A**: 图标需要 SVG 正确渲染，尝试点击状态栏按钮代替

### Q5: 命令面板没有 MatrixCode 命令？
**A**: 扩展可能未激活，手动运行命令触发激活

---

## 预期效果

成功后你应该看到：

1. **状态栏**: 底部左侧 🤖 MatrixCode 按钮
2. **活动栏**: 左侧图标栏有 MatrixCode 图标
3. **命令面板**: Ctrl+Shift+P 输入 "MatrixCode" 有命令列表
4. **输出日志**: MatrixCode 通道显示激活成功