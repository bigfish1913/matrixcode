# 设计方案: MatrixCode GUI 入口

日期: 2025-01-18

## 核心目标

- **降低使用门槛**: 让不熟悉命令行的用户也能方便使用 MatrixCode
- **增强可视化**: 提供更丰富的交互界面、历史记录、多窗口等功能
- **提供独立的桌面应用**: 不依赖编辑器，提供一致的桌面应用体验
- **跨平台统一体验**: 提供一致的桌面应用体验，与终端无关

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                    GUI Application                      │
├─────────────────────────────────────────────────────────┤
│  Frontend (React + TypeScript)                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │  UI Components                                   │    │
│  │  - ChatWindow (对话窗口)                        │    │
│  │  - SessionList (会话列表)                        │    │
│  │  - TaskPanel (任务面板)                          │    │
│  │  - ConfigPage (配置页面)                         │    │
│  └─────────────────────────────────────────────────┘    │
│                        │                                │
│                        │ Tauri invoke()                 │
│                        ▼                                │
│  ┌─────────────────────────────────────────────────┐    │
│  │  Tauri Commands (Rust)                          │    │
│  │  - start_session() → Agent::run()              │    │
│  │  - send_message() → Agent::send_message()      │    │
│  │  - get_history() → Session::load()            │    │
│  │  - get_config() → Config::load()              │    │
│  └─────────────────────────────────────────────────┘    │
│                        │                                │
│                        ▼                                │
│  ┌─────────────────────────────────────────────────┐    │
│  │  matrixcode-core (Shared Library)              │    │
│  │  - Agent (核心逻辑)                              │    │
│  │  - Session (会话管理)                           │    │
│  │  - Config (配置)                                │    │
│  │  - Providers (AI 提供商)                        │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

**核心设计：**
- Frontend 负责 UI 渲染和用户交互
- Tauri Commands 作为桥接层，暴露 Rust API 给前端
- 复用 matrixcode-core 的所有核心逻辑

**技术栈：**
- Tauri 2.x (最新稳定版)
- React 18 + TypeScript 5
- TailwindCSS + shadcn/ui (组件库)
- Zustand (状态管理)
- React Router (路由)

## 数据模型 / 核心实体

```typescript
// 会话
interface Session {
  id: string;
  title: string;
  createdAt: Date;
  updatedAt: Date;
  messages: Message[];
  metadata: SessionMetadata;
}

// 消息
interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: ContentBlock[];
  timestamp: Date;
}

// 内容块（支持多种类型）
type ContentBlock = 
  | { type: 'text'; text: string }
  | { type: 'tool_use'; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; content: string };

// 任务
interface Task {
  id: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  content: string;
  activeForm: string;
  result?: string;
}

// 配置
interface Config {
  provider: 'anthropic' | 'openai';
  apiKey: string;
  modelName: string;
  multiModel: MultiModelConfig;
  compressionThreshold: number;
  mcpServers: MCPServer[];
}

// 工具执行状态
interface ToolExecution {
  toolName: string;
  status: 'pending' | 'running' | 'success' | 'error';
  input: any;
  output?: any;
  duration?: number;
}
```

**状态管理设计：**
- 使用 Zustand 管理全局状态
- SessionStore：当前会话、会话列表
- TaskStore：任务进度、工具执行状态
- ConfigStore：配置管理
- UIStore：界面状态（侧边栏折叠、主题等）

## 关键接口 / API

**Tauri Commands（Rust 侧）：**

```rust
// 会话管理
#[tauri::command]
async fn create_session(title: Option<String>) -> Result<Session, String>;

#[tauri::command]
async fn load_session(id: String) -> Result<Session, String>;

#[tauri::command]
async fn list_sessions(page: u32, limit: u32) -> Result<Vec<SessionSummary>, String>;

#[tauri::command]
async fn delete_session(id: String) -> Result<(), String>;

// 对话交互
#[tauri::command]
async fn send_message(session_id: String, content: String) -> Result<(), String>;

#[tauri::command]
async fn stream_response(session_id: String) -> Result<(), String>; // 通过事件推送

#[tauri::command]
async fn stop_generation(session_id: String) -> Result<(), String>;

// 任务管理
#[tauri::command]
async fn get_task_status(session_id: String) -> Result<Vec<Task>, String>;

#[tauri::command]
async fn get_tool_executions(session_id: String) -> Result<Vec<ToolExecution>, String>;

// 配置管理
#[tauri::command]
async fn load_config() -> Result<Config, String>;

#[tauri::command]
async fn save_config(config: Config) -> Result<(), String>;

#[tauri::command]
async fn test_provider_connection() -> Result<bool, String>;
```

**事件系统（Rust → Frontend 推送）：**

```rust
// 流式响应事件
app_handle.emit("stream-event", StreamPayload {
    session_id,
    event_type: "text" | "tool_use" | "tool_result",
    content: "...",
});

// 任务进度事件
app_handle.emit("task-update", TaskPayload {
    session_id,
    task_id,
    status: "in_progress" | "completed",
    progress: 0.8,
});

// 工具执行事件
app_handle.emit("tool-execution", ToolPayload {
    tool_name,
    status: "running" | "success" | "error",
    output_preview: "...",
});
```

## 技术方案

- **方案选择**: Tauri + React + TypeScript
- **理由**: 
  - React 生态成熟，组件库丰富（shadcn/ui、Ant Design）
  - TypeScript 类型安全，减少前后端类型不一致问题
  - Tauri 打包体积小（~3MB vs Electron ~50MB）
  - 开发效率高，前后端分工明确

## 错误处理策略

**Rust 侧：**
- 统一错误类型 GuiError，使用 thiserror 定义
- Tauri Command 返回 Result<T, String>，错误自动序列化传递

**前端侧：**
- Error Boundary 捕获组件错误
- API 调用使用 try-catch + toast 提示
- 错误分类处理（API key、rate limit、network 等）
- 工具执行错误显示可折叠详情 + 重试按钮

## 测试策略

**Rust 后端测试：**
- 单元测试：核心命令逻辑，使用 mock provider
- 集成测试：Tauri Commands，使用 tauri::test::mock_app
- 覆盖率目标：> 80%

**前端测试：**
- 组件测试：Jest + React Testing Library
- 状态管理测试：Zustand store 测试
- 覆盖率目标：> 70%

**端到端测试：**
- E2E 测试：Playwright
- 核心流程：创建会话 → 发送消息 → 查看响应 → 管理配���
- 覆盖率目标：核心用户流程 100%

## 项目结构

```
packages/gui/
├── src/                      # React 前端代码
│   ├── components/           # UI 组件
│   │   ├── Chat/            # 对话相关组件
│   │   ├── Session/         # 会话管理组件
│   │   ├── Task/            # 任务面板组件
│   │   ├── Config/          # 配置页面组件
│   │   └── common/          # 通用组件
│   ├── stores/              # Zustand 状态管理
│   ├── api/                 # Tauri API 封装
│   ├── hooks/               # 自定义 Hooks
│   ├── utils/               # 工具函数
│   ├── styles/              # 样式文件
│   ├── App.tsx              # 根组件
│   └── main.tsx             # 入口文件
│
├── src-tauri/               # Rust 后端代码
│   ├── src/
│   │   ├── commands/        # Tauri Commands
│   │   ├── events/          # 事件定义
│   │   ├── state/           # 应用状态管理
│   │   ├── error.rs         # 错误处理
│   │   ├── lib.rs           # 库入口
│   │   └── main.rs          # Tauri 应用入口
│   ├── Cargo.toml           # Rust 依赖
│   ├── tauri.conf.json      # Tauri 配置
│   └── build.rs             # 构建脚本
│
├── tests/                   # 测试文件
│   ├── unit/               # 单元测试
│   └── e2e/                # 端到端测试
│
├── package.json             # Node.js 依赖
├── vite.config.ts          # Vite 配置
├── tsconfig.json           # TypeScript 配置
├── tailwind.config.js      # Tailwind 配置
└── README.md               # GUI 文档
```

**Workspace 集成：**

```toml
# Cargo.toml (workspace 根)
[workspace]
members = [
    "core",
    "cli",
    "tui",
    "packages/gui/src-tauri",  # 新增 GUI package
]
```

## 约束与风险

**约束：**
- 必须复用 matrixcode-core，不能重复实现核心逻辑
- 必须支持跨平台（Windows/macOS/Linux）
- 打包体积必须 < 20MB（Tauri 优势）

**风险与应对：**

1. **Tauri 2.x 稳定性风险**
   - 应对：使用 2.0 稳定版，参考官方示例和最佳实践

2. **前后端类型同步风险**
   - 应对：使用 ts-rs 自动生成 TypeScript 类型定义

3. **流式响应性能风险**
   - 应对：使用 Tauri Events 推送，避免轮询

4. **开发周期风险**
   - 应对：分阶段实现，优先对话核心功能，逐步完善

## 验收标准

- 完整实现所有核心功能：
  - 对话交互（发送消息、流���响应、工具执行展示）
  - 任务管理（进度显示、工具执行日志）
  - 会话历史（浏览、恢复、删除）
  - 配置管理（Provider、模型参数、MCP 工具）

- 测试覆盖率达到目标：
  - Rust 后端 > 80%
  - 前端组件 > 70%
  - E2E 核心流程 100%

- 能独立运行，不依赖 CLI/TUI

- 打包体积 < 20MB（Windows/macOS/Linux）

- 提供完整的用户文档和开发文档