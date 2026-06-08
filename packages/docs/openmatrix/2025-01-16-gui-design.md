# 设计方案: MatrixCode 桌面 GUI

日期: 2025-01-16

## 核心目标

- GUI 作为主要界面，CLI 作为备选和自动化场景
- 核心对话界面：消息展示、工具调用、流式响应
- 项目管理：文件树、符号导航、项目概览
- 代码编辑/查看：语法高亮、diff 展示、代码浏览
- 任务管理：进度追踪、历史会话、设置管理

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Application                     │
├─────────────────────────────────────────────────────────┤
│  Frontend (WebView)                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   React     │  │   React     │  │   React     │    │
│  │   Chat      │  │   Project   │  │   Editor    │    │
│  │   View      │  │   Manager   │  │   View      │    │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘    │
│         │                │                │            │
│         └────────────────┼────────────────┘            │
│                          │                             │
│                   Tauri Commands                      │
│                   (IPC Bridge)                        │
└──────────────────────────┬────────────────────────────┘
                           │
┌──────────────────────────┴────────────────────────────┐
│              Rust Backend (packages/core)             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   Agent     │  │   Provider  │  │   Session   │  │
│  │   Loop     │  │   (LLM API) │  │   Manager   │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   Tools     │  │   Memory    │  │   Workflow  │  │
│  │   (Bash/Ed) │  │   Storage   │  │   Engine    │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
└───────────────────────────────────────────────────────┘
```

**架构说明：**

1. **Frontend Layer（WebView）**
   - React 18 + TypeScript + Tailwind CSS
   - 四个核心视图：Chat、Project Manager、Editor、Task Manager
   - React Query 管理服务端状态
   - Zustand/Jotai 管理客户端状态

2. **Communication Layer**
   - Tauri Commands 作为 IPC 桥梁
   - 前端通过 `invoke()` 调用后端命令
   - 后端通过 `emit()` 推送事件（流式响应、进度更新）

3. **Backend Layer（复用现有 Core）**
   - 复用 packages/core 的所有模块
   - Agent Loop、Provider、Tools、Memory、Workflow 保持不变
   - 新增 Tauri Commands 适配层

## 数据模型 / 核心实体

**前端核心状态：**

```typescript
// 会话状态
interface Session {
  id: string;
  projectId?: string;
  messages: Message[];
  status: 'idle' | 'running' | 'waiting_for_input';
  createdAt: Date;
  updatedAt: Date;
}

// 消息
interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: ContentBlock[];
  timestamp: Date;
}

// 内容块
type ContentBlock = 
  | { type: 'text'; text: string }
  | { type: 'tool_use'; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; content: string }

// 项目
interface Project {
  id: string;
  path: string;
  name: string;
  techStack: string[];
  lastOpened: Date;
}

// 任务
interface Task {
  id: string;
  title: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  sessionId?: string;
}
```

**后端核心实体（复用现有）：**

- `Message` - 消息结构
- `ContentBlock` - 内容块
- `Session` - 会话管理
- `Project` - 项目元数据
- `Task` - 任务状态

## 关键接口 / API

**Tauri Commands（后端暴露给前端）：**

```rust
// 会话管理
#[tauri::command]
async fn create_session(project_id: Option<String>) -> Result<Session, String>;

#[tauri::command]
async fn send_message(session_id: String, content: String) -> Result<(), String>;

#[tauri::command]
async fn get_session_history(session_id: String) -> Result<Vec<Message>, String>;

// 项目管理
#[tauri::command]
async fn open_project(path: String) -> Result<Project, String>;

#[tauri::command]
async fn get_project_tree(project_id: String) -> Result<FileTree, String>;

#[tauri::command]
async fn read_file(project_id: String, path: String) -> Result<String, String>;

#[tauri::command]
async fn write_file(project_id: String, path: String, content: String) -> Result<(), String>;

// Agent 控制
#[tauri::command]
async fn cancel_task(session_id: String) -> Result<(), String>;

#[tauri::command]
async fn pause_task(session_id: String) -> Result<(), String>;

#[tauri::command]
async fn resume_task(session_id: String) -> Result<(), String>;

// 任务管理
#[tauri::command]
async fn get_tasks() -> Result<Vec<Task>, String>;

#[tauri::command]
async fn get_task_status(task_id: String) -> Result<TaskStatus, String>;
```

**事件流（后端推送给前端）：**

```rust
// 流式响应
app.emit("message_chunk", &MessageChunk { session_id, delta });

// 工具调用
app.emit("tool_call", &ToolCall { session_id, tool_name, input });

// 进度更新
app.emit("progress", &Progress { session_id, current, total, message });

// 错误通知
app.emit("error", &Error { session_id, message, details });
```

## 技术方案

- **方案选择：** Tauri + React + TypeScript
- **理由：**
  - React 生态成熟，UI 组件库丰富（shadcn/ui、Ant Design）
  - TypeScript 类型安全，与 Rust 类型系统契合
  - 开发效率高，热重载、调试工具完善
  - Tauri 包体积小、性能好，适合构建现代桌面应用

## 错误处理策略

**后端错误处理（Rust）：**

```rust
// 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum MatrixCodeError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Project error: {0}")]
    ProjectError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Provider error: {0}")]
    ProviderError(String),
    
    #[error("Tool execution failed: {0}")]
    ToolError(String),
}

// 错误转换为 Tauri 响应
impl From<MatrixCodeError> for String {
    fn from(error: MatrixCodeError) -> Self {
        error.to_string()
    }
}
```

**前端错误处理：**

1. **全局错误边界**
   - React Error Boundary 捕获渲染错误
   - 显示友好错误页面，提供重试选项

2. **API 错误处理**
   - React Query 自动重试（指数退避）
   - Toast 通知显示错误信息
   - 网络断开时自动重连

3. **用户反馈**
   - Loading 状态指示器
   - 错误消息本地化
   - 错误日志记录到本地文件

## 测试策略

**后端测试：**

```rust
// 单元测试
#[cfg(test)]
mod tests {
    #[test]
    fn test_create_session() { /* ... */ }
    
    #[test]
    fn test_send_message() { /* ... */ }
}

// 集成测试
#[tokio::test]
async fn test_full_conversation_flow() {
    let app = MockApp::new();
    let session = create_session(None).await.unwrap();
    send_message(session.id, "Hello").await.unwrap();
    // ...
}
```

**前端测试：**

1. **单元测试**
   - 状态管理逻辑测试
   - 工具函数测试

2. **组件测试**
   - React Testing Library
   - 用户交互测试

3. **E2E 测试（Playwright）**
   - 完整用户流程测试
   - 跨平台测试（Windows/macOS/Linux）

**测试覆盖率目标：**
- 后端核心逻辑：80%+
- 前端组件：60%+
- E2E 覆盖关键流程

## 约束与风险

**约束：**
- 必须复用现有 packages/core，不重复实现
- 前端技术栈学习成本（如团队不熟悉 React/TypeScript）
- 打包体积控制（避免过大）

**风险及应对：**
- **风险1：前端技术栈不熟悉**
  - 应对：使用成熟的组件库（shadcn/ui），降低开发难度
  
- **风险2：IPC 通信性能瓶颈**
  - 应对：批量传输大文件，流式传输消息
  
- **风险3：跨平台兼容性问题**
  - 应对：Tauri 自动处理大部分差异，E2E 测试覆盖多平台

## 验收标准

- 四大核心功能完整可用（对话、项目管理、代码编辑��任务管理）
- 会话持久化和恢复
- 流式响应实时展示
- 工具调用可视化
- 错误处理友好（Toast 通知、错误边界）
- 测试覆盖率达标（后端 80%+、前端 60%+）
- 打包体积 < 20MB
- 启动时间 < 3s