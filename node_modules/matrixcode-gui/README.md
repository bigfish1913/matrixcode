# MatrixCode GUI

MatrixCode 桌面 GUI 应用，基于 Tauri + React + TypeScript。

## 技术栈

- **Frontend**: React 18 + TypeScript 5 + Tailwind CSS 3
- **Backend**: Tauri 2.x + Rust
- **Build**: Vite 5

## 开发

### 安装依赖

```bash
npm install
```

### 启动开发模式

```bash
npm run tauri:dev
```

这将同时启动前端开发服务器和 Tauri 应用。

### 构建

```bash
npm run tauri:build
```

## 项目结构

```
gui/
├── src/                # React 前端源码
├── src-tauri/          # Tauri Rust 后端
│   ├── src/            # Rust 源码
│   ├── Cargo.toml      # Rust 依赖配置
│   └── tauri.conf.json # Tauri 配置
├── public/             # 静态资源
├── tests/              # 前端测试
├── package.json        # Node 依赖配置
├── vite.config.ts      # Vite 配置
├── tsconfig.json       # TypeScript 配置
└── tailwind.config.js  # Tailwind CSS 配置
```

## 功能

- 对话界面（Chat View）
- 项目管理（Project Manager）
- 代码编辑（Editor View）
- 任务管理（Task Manager）
- 设置面板（Settings Panel）

## 文档

- [架构文档](../../docs/openmatrix/2025-01-18-gui-design.md)
- [开发指南](../../docs/gui-development.md)
- [API 文档](../../docs/gui-api.md)

## 许可证

MIT