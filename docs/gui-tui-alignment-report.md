# GUI 功能对齐 TUI 优化报告

## 优化概述

本次优化主要针对 MatrixCode GUI 界面，使其功能更好地对齐 TUI (Terminal UI) 的功能，提供更完整的用户体验。

## 新增功能

### 1. LSP 状态面板 (LspStatusPanel)
- **功能**: 显示 LSP (Language Server Protocol) 服务器状态
- **对应 TUI**: `lsp_servers` 字段
- **快捷键**: `Alt+L` 或 `/lsp` 命令
- **特性**:
  - 显示服务器名称、语言、状态（运行/停止/错误）
  - 显示服务器命令和错误信息
  - 支持刷新状态

### 2. CodeGraph 状态面板 (CodeGraphStatusPanel)
- **功能**: 显示 CodeGraph 索引状态和统计信息
- **对应 TUI**: `codegraph_status` 字段
- **快捷键**: `Alt+G` 或 `/codegraph` 命令
- **特性**:
  - 显示初始化状态和索引进度
  - 显示文件、符号、边的统计数量
  - 显示待同步文件列表
  - 支持初始化和重新索引操作
  - 显示错误信息

### 3. 循环任务指示器 (LoopTaskIndicator)
- **功能**: 显示循环任务和 Cron 任务状态
- **对应 TUI**: `loop_task` 和 `cron_tasks` 字段
- **特性**:
  - 显示循环任务的消息、间隔、计数
  - 显示 Cron 任务列表和间隔
  - 支持停止任务操作
  - 显示进度条（对于有最大计数的任务）

## 技术改进

### 前端改进 (chatStore.ts)
- 新增状态字段:
  - `lspServers`: LSP 服务器列表
  - `codeGraphStatus`: CodeGraph 状态
  - `loopTask`: 循环任务状态
  - `cronTasks`: Cron 任务列表
- 新增方法:
  - `updateLspServers`: 更新 LSP 状态
  - `updateCodeGraphStatus`: 更新 CodeGraph 状态
  - `updateLoopTask`: 更新循环任务
  - `updateCronTasks`: 更新 Cron 任务
  - `stopLoopTask`: 停止循环任务
  - `stopCronTask`: 停止特定 Cron 任务

### 后端改进 (lib.rs)
- 新增 Tauri 命令:
  - `get_lsp_status`: 获取 LSP 状态
  - `get_codegraph_status`: 获取 CodeGraph 状态
  - `initialize_codegraph`: 初始化 CodeGraph
  - `reindex_codegraph`: 重新索引 CodeGraph
- 新增数据结构:
  - `LspServerInfo`: LSP 服务器信息
  - `CodeGraphStatus`: CodeGraph 状态信息

### 快捷键改进 (ShortcutHelp.tsx)
- 新增快捷键说明:
  - `Alt+L`: LSP 服务器状态
  - `Alt+G`: CodeGraph 状态
  - `/lsp`: LSP 命令
  - `/codegraph`: CodeGraph 命令

## 文件修改清单

### 新增文件
1. `packages/gui/src/components/LspStatusPanel.tsx`
2. `packages/gui/src/components/CodeGraphStatusPanel.tsx`
3. `packages/gui/src/components/LoopTaskIndicator.tsx`

### 修改文件
1. `packages/gui/src/stores/chatStore.ts` - 新增状态和方法
2. `packages/gui/src/components/ChatView.tsx` - 集成新组件和快捷键
3. `packages/gui/src-tauri/src/lib.rs` - 新增后端 API 命令
4. `packages/gui/src/components/ShortcutHelp.tsx` - 新增快捷键说明
5. `packages/gui/src/components/I18n.tsx` - 修复 useEffect 返回值问题

## 下一步工作

### 后端集成
- 将 LSP 状态命令与实际 LSP 管理器集成
- 将 CodeGraph 命令与实际 CodeGraph 工具集成
- 实现循环任务和 Cron 任务的后端支持

### 功能增强
- 添加 LSP 服务器启动/停止控制
- 添加 CodeGraph 实时进度显示
- 支持循环任务的暂停/恢复功能
- 添加任务历史记录

### 测试
- 添加组件单元测试
- 添加后端 API 测试
- 添加用户界面测试

## 构建状态

✅ TypeScript 编译通过
✅ Vite 构建成功
⚠️ 存在 chunk 大小警告（可优化代码分割）

## 总结

本次优化成功地将 TUI 的核心状态监控功能迁移到 GUI，提供了更直观的界面展示。用户现在可以通过面板和快捷键快速查看 LSP、CodeGraph 和任务状态，大大提升了使用体验。