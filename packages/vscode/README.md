# MatrixCode VSCode Extension

## 开发与调试

### 快速开始

1. **安装依赖**
   ```bash
   npm install
   ```

2. **开发编译（带 sourcemap）**
   ```bash
   npm run compile-dev
   ```

3. **启动调试**
   - 在 VSCode 中打开此目录
   - 按 `F5` 启动调试
   - 会自动打开一个新的 VSCode 窗口（扩展开发宿主）
   - 在侧边栏可以看到 MatrixCode 图标

### 调试配置

本项目提供三种调试模式：

| 配置名称 | 说明 | 使用场景 |
|---------|------|---------|
| Run Extension (Debug) | 单次编译后调试 | 快速测试 |
| Run Extension (Watch) | 监听模式调试 | 持续开发 |
| Extension Tests | 运行测试 | 测试验证 |

### 调试技巧

1. **设置断点**
   - 在源代码中点击行号左侧设置断点
   - 调试时会自动停在断点处

2. **查看变量**
   - 使用调试面板查看变量值
   - 使用 `console.log` 输出到调试控制台

3. **热重载**
   - 使用 "Run Extension (Watch)" 配置
   - 修改代码后，在扩展宿主窗口按 `Ctrl+R` (Windows) 或 `Cmd+R` (Mac) 重载

4. **查看日志**
   - 在扩展宿主窗口按 `Ctrl+Shift+U` 打开输出面板
   - 选择 "MatrixCode" 查看扩展日志

### 常用命令

```bash
# 开发编译（带 sourcemap）
npm run compile-dev

# 监听模式（自动编译）
npm run watch

# 生产编译（压缩）
npm run compile

# 代码检查
npm run lint

# 打包 VSIX
npm run package

# 发布到 Marketplace
npm run publish
```

### 项目结构

```
packages/vscode/
├── src/
│   ├── extension.ts      # 扩展入口
│   ├── chatView.ts       # 聊天视图
│   ├── configManager.ts  # 配置管理
│   └── matrixcodeClient.ts # CLI 客户端
├── dist/                 # 编译输出
├── resources/            # 资源文件
└── .vscode/              # VSCode 配置
    ├── launch.json       # 调试配置
    ├── tasks.json        # 任务配置
    └── settings.json     # 工作区设置
```

### 相关链接

- [VSCode 扩展开发文档](https://code.visualstudio.com/api)
- [esbuild 文档](https://esbuild.github.io/)
- [ESLint 文档](https://eslint.org/)