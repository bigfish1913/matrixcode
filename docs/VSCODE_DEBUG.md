# VSCode Extension Debug Guide

## 🎯 如何调试 MatrixCode VSCode 扩展

### 1. 快速调试步骤

```bash
# 在 packages/vscode 目录下
cd packages/vscode

# 1. 安装依赖（如果未安装）
npm install

# 2. 编译开发版本（带 sourcemap）
npm run compile-dev

# 3. 在 VSCode 中打开此目录
code .

# 4. 按 F5 启动调试
```

### 2. 调试模式说明

#### Run Extension (Debug)
- **用途**: 单次编译后快速测试
- **特点**: 
  - 自动编译（compile-dev）
  - 生成 sourcemap，可以设置断点
  - 启动新的 VSCode 窗口（扩展开发宿主）

#### Run Extension (Watch) ⭐ 推荐
- **用途**: 持续开发和调试
- **特点**:
  - 监听文件变化，自动编译
  - 在扩展宿主窗口按 Ctrl+R 重载
  - 实时看到代码修改效果

#### Extension Tests
- **用途**: 运行测试
- **特点**: 
  - 运行扩展测试套件
  - 验证功能正确性

### 3. 调试技巧

#### 设置断点
```
1. 打开 src/extension.ts
2. 在第 14 行点击行号左侧
3. 出现红色圆点 = 断点设置成功
4. 按 F5 启动调试
5. 触发断点时，调试器自动暂停
```

#### 查看变量
- **调试面板**: 左侧变量面板查看当前变量
- **调试控制台**: 在控制台输入变量名��看值
- **console.log**: 输出到扩展宿主的输出面板

#### 热重载（推荐）
```
1. 选择 "Run Extension (Watch)" 配置
2. 按 F5 启动
3. 修改代码（如 src/chatView.ts）
4. 在扩展宿主窗口按 Ctrl+R 重载
5. 立即看到修改效果
```

#### 查看日志
```
1. 在扩展宿主窗口
2. 按 Ctrl+Shift+U 打开输出面板
3. 选择 "MatrixCode" 下拉选项
4. 查看扩展输出的日志
```

### 4. 测试扩展功能

#### 测试聊天视图
1. 启动调试后，在扩展宿主窗口
2. 点击左侧活动栏的 MatrixCode 图标
3. 在聊天框输入消息测试

#### 测试命令
```
1. Ctrl+Shift+P 打开命令面板
2. 输入 "MatrixCode" 查看所有命令
3. 选择命令测试：
   - MatrixCode: Explain Code
   - MatrixCode: Fix Code
   - MatrixCode: Generate Tests
   - MatrixCode: Refactor
```

### 5. 常见问题

#### Q: 按 F5 没反应？
**A**: 确保在 packages/vscode 目录下打开 VSCode，而不是根目录

#### Q: 看不到 MatrixCode 图标？
**A**: 检查 dist/extension.js 是否存在，运行 `npm run compile-dev`

#### Q: 断点不生效？
**A**: 确保使用 compile-dev（带 sourcemap），不要用 compile（压缩版）

#### Q: 扩展宿主窗口报错？
**A**: 查看输出面板的 "MatrixCode" 日志，或检查 CLI 是否已构建

### 6. 使用工作区文件

```bash
# 在根目录打开工作区
code matrixcode.code-workspace

# 可以同时调试 CLI 和 VSCode 扩展
# CLI: 在 CLI 文件夹运行 cargo run
# VSCode: 在 VSCode Extension 文件夹按 F5
```

### 7. 快捷键

| 快捷键 | 功能 |
|--------|------|
| F5 | 启动调试 |
| Ctrl+Shift+F5 | 重启调试 |
| Shift+F5 | 停止调试 |
| F9 | 设置/取消断点 |
| F10 | 单步跳过 |
| F11 | 单步进入 |
| Shift+F11 | 单步退出 |
| Ctrl+R | 重载扩展（在扩展宿主窗口）|

---

## 📚 更多资源

- [VSCode Extension API](https://code.visualstudio.com/api)
- [Extension Development Overview](https://code.visualstudio.com/api/get-started/your-first-extension)
- [Testing Extensions](https://code.visualstudio.com/api/working-with-extensions/testing)
- [Debugging Extensions](https://code.visualstudio.com/api/working-with-extensions/debugging)