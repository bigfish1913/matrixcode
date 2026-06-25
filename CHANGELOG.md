# 更新日志

所有重要的改动都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 新增
- VSCode 扩展框架 (`packages/vscode/`)
- CLI JSON 输出模式 (`--json`)
- CLI Daemon 模式 (`--daemon`)
- IPC 协议模块 (`src/protocol.rs`, `src/ipc.rs`)
- Monorepo 项目结构

### 改进
- 重构项目为 packages/cli 和 packages/vscode
- 更新 CI/CD 工作流适配新结构
- 添加 ESLint 配置
- 添加开发脚本 (scripts/setup.sh, scripts/setup.bat)
- 添加 Makefile

## [0.4.46] - 2026-06-25

### 修复 🐛
- **Session 压缩功能完全失效** - 修复严重 bug (#1)
  - Agent 现正确区分 `full_messages`（用于显示）和 `messages`（用于 API）
  - Session 保存正确区分完整消息和压缩消息
  - 添加压缩历史记录，追踪每次压缩的详细信息
  - AgentBuilder 新增 `initial_messages` 方法支持消息恢复

### 性能改进 ⚡
- Token 使用量预计减少 **60%+**
- Session 文件大小预计减少 **50%+**
- API 成本预计降低 **60%+**
- 响应速度显著提升（更小的上下文窗口）

### 技术细节 🔧
- Agent 结构新增 `full_messages` 字段 (types.rs)
- 压缩逻辑改进：压缩前保存完整消息，只压缩 API 消息 (run.rs)
- Session 保存逻辑重构：正确区分两种消息类型 (session.rs)
- Agent 创建优化：使用 builder 模式直接设置初始消息 (agent.rs)

## [0.2.5] - 2024-05-16

### 新增
- IPC 消息类型定义 (`protocol.rs`)
- Daemon 模式实现 (`ipc.rs`)
- Agent JSON 流式输出方法 (`chat_stream_json`)
- VSCode 扩展设计文档

### 改进
- CLI 参数支持 `--json` 和 `--daemon`

## [0.2.4] - 2024-05-15

### 改进
- 优化上下文压缩策略
- 改进记忆系统性能

## [0.2.3] - 2024-05-14

### 新增
- 跨会话记忆系统
- 自动记忆累积和加载

### 改进
- 增强会话管理功能

## [0.2.2] - 2024-05-13

### 新增
- 多模型配置支持 (main/plan/compress/fast)
- 任务规划功能

### 改进
- 优化 API 调用效率

## [0.2.1] - 2024-05-12

### 新增
- 项目概览生成 (`--init`)
- 技能系统

### 改进
- 完善 REPL 命令
- 添加压缩偏重选项

## [0.2.0] - 2024-05-10

### 新增
- 上下文压缩功能
- 会话持久化管理
- Web 搜索能力
- 工具审批机制

### 改进
- 重构 Agent 核心
- 优化流式输出

## [0.1.0] - 2024-05-01

### 新增
- 基本 CLI 功能
- Anthropic 和 OpenAI Provider
- 工具系统 (read/write/edit/bash/search 等)
- REPL 交互模式
- 单次问答模式

---

## 版本说明

- **[Unreleased]**: 开发中的功能
- **[0.2.5]**: VSCode 扩展集成准备
- **[0.2.0]**: 核心功能完善版本
- **[0.1.0]**: 首个可用版本