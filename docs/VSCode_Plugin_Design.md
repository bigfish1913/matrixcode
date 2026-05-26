# MatrixCode VSCode 插件设计方案

## 概述

本文档描述如何将 MatrixCode CLI 集成为 VSCode 插件，采用 **CLI 集成模式**。

## 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                    VSCode Extension (TypeScript)                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Sidebar   │  │  Status Bar │  │   Webview Panel          │ │
│  │   Chat View │  │  Indicator  │  │   (Markdown Rendering)   │ │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘ │
│         │                │                     │                 │
│         └────────────────┼─────────────────────┘                 │
│                          │                                       │
│                  ┌───────▼───────┐                               │
│                  │  IPC Client   │                               │
│                  │  (JSON-RPC)   │                               │
│                  └───────┬───────┘                               │
└──────────────────────────┼──────────────────────────────────────┘
                           │
                           │ stdin/stdout (JSON Lines)
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                    MatrixCode CLI (Rust)                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │  IPC Mode   │  │  JSON I/O   │  │   Core Agent Logic      │ │
│  │  --daemon   │  │  --json     │  │   (unchanged)           │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 通信协议

### 启动模式

```bash
# 方式1: 单次问答模式 (推荐用于 MVP)
matrixcode --json "你的问题"

# 方式2: 持久 daemon 模式 (推荐用于正式版)
matrixcode --daemon --port 9527
```

### JSON 消息格式

#### 请求格式 (单次模式)

```json
{
  "type": "chat",
  "content": "帮我分析这个函数",
  "context": {
    "file": "src/main.rs",
    "selection": {
      "start": { "line": 10, "character": 0 },
      "end": { "line": 20, "character": 0 }
    },
    "workspace": "/path/to/project"
  }
}
```

#### 响应格式 (流式)

```json
{"type": "text", "content": "这是一个"}
{"type": "text", "content": "简单的函数"}
{"type": "tool_use", "id": "tool_1", "name": "read", "input": {"path": "src/main.rs"}}
{"type": "tool_result", "tool_use_id": "tool_1", "content": "文件内容..."}
{"type": "thinking", "content": "正在分析..."}
{"type": "done", "usage": {"input": 1234, "output": 567}}
```

#### Daemon 模式协议 (JSON-RPC 2.0)

```json
// 请求
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "chat",
  "params": {
    "message": "你的问题",
    "context": { ... }
  }
}

// 响应 (流式事件)
{
  "jsonrpc": "2.0",
  "method": "stream",
  "params": {"type": "text", "content": "..."}
}

// 最终响应
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "status": "completed",
    "usage": {"input": 1234, "output": 567}
  }
}
```

## 文件结构

```
matrixcode/
├── packages/
│   ├── cli/                    # Rust CLI
│   │   ├── src/
│   │   │   ├── main.rs        # --json 和 --daemon 参数
│   │   │   ├── ipc.rs         # IPC 通信模块
│   │   │   ├── protocol.rs    # 消息类型定义
│   │   │   └── ...
│   │   └── Cargo.toml
│   │
│   └── vscode/                 # VSCode 插件
│       ├── package.json        # 插件配置
│       ├── tsconfig.json       # TypeScript 配置
│       ├── src/
│       │   ├── extension.ts    # 插件入口
│       │   ├── chatView.ts     # 侧边栏聊天界面
│       │   ├── matrixcodeClient.ts  # CLI 通信客户端
│       │   ├── configManager.ts     # 配置管理
│       │   ├── contextProvider.ts   # VSCode 上下文提取
│       │   └── utils/
│       │       ├── markdown.ts      # Markdown 渲染
│       │       └── fileOps.ts       # 文件操作辅助
│       ├── webview/            # WebView UI
│       │   ├── index.html
│       │   ├── main.ts
│       │   └── styles.css
│       └── README.md
│
└── docs/
    └── VSCode_Plugin_Design.md  # 本文档
```

## 实现步骤

### 第一阶段: CLI 改造 (MVP)

1. **添加 `--json` 输出模式**
   - 修改 `src/main.rs`，添加 `--json` 参数
   - 输出格式改为 JSON Lines (流式)
   - 保持向后兼容，默认行为不变

2. **添加 `--daemon` 模式**
   - 创建 `src/ipc.rs` 模块
   - 支持 stdin/stdout JSON-RPC 通信
   - 可选: 支持 TCP 端口监听

3. **定义消息类型**
   - 创建 `src/protocol.rs` 定义消息结构
   - 使用 serde 序列化/反序列化

### 第二阶段: VSCode 插件开发

1. **基础框架**
   - 创建 vscode 目录
   - 配置 TypeScript + esbuild
   - 实现插件激活/停用

2. **MatrixCode 客户端**
   - 实现 CLI 进程管理
   - JSON 消息解析
   - 错误处理和重连

3. **聊天界面**
   - 侧边栏 Chat View
   - WebView 消息渲染
   - Markdown 支持

4. **上下文集成**
   - 当前文件/选择提取
   - 工作区信息
   - 错误/诊断信息

### 第三阶段: 功能增强

1. **代码操作**
   - 右键菜单集成
   - 快速修复建议
   - 代码解释

2. **会话管理**
   - 会话列表
   - 会话恢复
   - 记忆管理

3. **配置界面**
   - 模型选择
   - API Key 配置
   - 偏好设置

## 详细实现

### CLI 改造: protocol.rs

```rust
// src/protocol.rs

use serde::{Deserialize, Serialize};

/// VSCode 插件请求
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientRequest {
    #[serde(rename = "type")]
    pub request_type: RequestType,
    pub content: String,
    #[serde(default)]
    pub context: Option<RequestContext>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestType {
    Chat,
    QuickAction,
    ExplainCode,
    FixCode,
    GenerateTests,
    Refactor,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestContext {
    pub workspace: Option<String>,
    pub file: Option<String>,
    pub selection: Option<Selection>,
    pub diagnostics: Option<Vec<Diagnostic>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub range: Selection,
}

/// CLI 响应 (流式)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    Text { content: String },
    Thinking { content: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
    Error { message: String },
    Done { usage: Usage },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
}
```

### CLI 改造: main.rs 新增参数

```rust
// 在 Cli 结构体中添加

/// Output in JSON format for IDE integration.
#[arg(long)]
json: bool,

/// Run as a daemon process communicating via stdin/stdout.
#[arg(long)]
daemon: bool,

/// Port for daemon mode (default: use stdin/stdout).
#[arg(long)]
port: Option<u16>,
```

### VSCode 插件: package.json

```json
{
  "name": "matrixcode",
  "displayName": "MatrixCode - AI Code Agent",
  "description": "Intelligent code agent with multi-model support and context awareness",
  "version": "0.1.0",
  "publisher": "bigfish1913",
  "engines": {
    "vscode": "^1.85.0"
  },
  "categories": ["Programming Languages", "Machine Learning", "Other"],
  "activationEvents": ["onStartupFinished"],
  "main": "./dist/extension.js",
  "contributes": {
    "viewsContainers": {
      "activitybar": [
        {
          "id": "matrixcode",
          "title": "MatrixCode",
          "icon": "resources/icon.svg"
        }
      ]
    },
    "views": {
      "matrixcode": [
        {
          "type": "webview",
          "id": "matrixcode.chat",
          "name": "Chat"
        }
      ]
    },
    "commands": [
      {
        "command": "matrixcode.explain",
        "title": "MatrixCode: Explain Code"
      },
      {
        "command": "matrixcode.fix",
        "title": "MatrixCode: Fix Code"
      },
      {
        "command": "matrixcode.generateTests",
        "title": "MatrixCode: Generate Tests"
      },
      {
        "command": "matrixcode.refactor",
        "title": "MatrixCode: Refactor"
      },
      {
        "command": "matrixcode.newSession",
        "title": "MatrixCode: New Session"
      }
    ],
    "menus": {
      "editor/context": [
        {
          "command": "matrixcode.explain",
          "when": "editorHasSelection",
          "group": "matrixcode@1"
        },
        {
          "command": "matrixcode.fix",
          "when": "editorHasSelection",
          "group": "matrixcode@2"
        },
        {
          "command": "matrixcode.refactor",
          "when": "editorHasSelection",
          "group": "matrixcode@3"
        }
      ]
    },
    "configuration": {
      "title": "MatrixCode",
      "properties": {
        "matrixcode.cliPath": {
          "type": "string",
          "default": "matrixcode",
          "description": "Path to MatrixCode CLI binary"
        },
        "matrixcode.provider": {
          "type": "string",
          "enum": ["anthropic", "openai"],
          "default": "anthropic",
          "description": "LLM provider"
        },
        "matrixcode.model": {
          "type": "string",
          "default": "claude-sonnet-4-20250514",
          "description": "Model name"
        },
        "matrixcode.autoContext": {
          "type": "boolean",
          "default": true,
          "description": "Automatically include file context"
        }
      }
    }
  },
  "scripts": {
    "vscode:prepublish": "npm run compile",
    "compile": "esbuild ./src/extension.ts --bundle --outfile=dist/extension.js --external:vscode --format=cjs --platform=node",
    "watch": "esbuild ./src/extension.ts --bundle --outfile=dist/extension.js --external:vscode --format=cjs --platform=node --watch"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "@types/vscode": "^1.85.0",
    "esbuild": "^0.20.0",
    "typescript": "^5.3.0"
  }
}
```

### VSCode 插件: extension.ts

```typescript
// vscode/src/extension.ts

import * as vscode from 'vscode';
import { MatrixCodeClient } from './matrixcodeClient';
import { ChatViewProvider } from './chatView';

let client: MatrixCodeClient;
let chatProvider: ChatViewProvider;

export async function activate(context: vscode.ExtensionContext) {
    console.log('MatrixCode extension is activating...');

    // Initialize CLI client
    const config = vscode.workspace.getConfiguration('matrixcode');
    const cliPath = config.get<string>('cliPath') || 'matrixcode';
    client = new MatrixCodeClient(cliPath);

    // Register chat view
    chatProvider = new ChatViewProvider(context.extensionUri, client);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('matrixcode.chat', chatProvider)
    );

    // Register commands
    registerCommands(context, client);

    // Check CLI availability
    const available = await client.checkAvailability();
    if (!available) {
        vscode.window.showWarningMessage(
            'MatrixCode CLI not found. Please install it: npm install -g @bigfishnpm/matrixcode',
            'Install'
        ).then(selection => {
            if (selection === 'Install') {
                vscode.env.openExternal(
                    vscode.Uri.parse('https://github.com/bigfish1913/matrixcode#installation')
                );
            }
        });
    }
}

function registerCommands(context: vscode.ExtensionContext, client: MatrixCodeClient) {
    // Explain code
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.explain', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) return;

            const selection = editor.selection;
            const text = editor.document.getText(selection);
            const file = editor.document.uri.fsPath;

            await chatProvider.sendMessage('explain', text, {
                file,
                language: editor.document.languageId
            });
        })
    );

    // Fix code
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.fix', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) return;

            const selection = editor.selection;
            const text = editor.document.getText(selection);

            await chatProvider.sendMessage('fix', text, {
                file: editor.document.uri.fsPath
            });
        })
    );

    // Generate tests
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.generateTests', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) return;

            const text = editor.document.getText();

            await chatProvider.sendMessage('generateTests', text, {
                file: editor.document.uri.fsPath,
                language: editor.document.languageId
            });
        })
    );

    // New session
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.newSession', async () => {
            await client.newSession();
            chatProvider.clearHistory();
            vscode.window.showInformationMessage('MatrixCode: New session started');
        })
    );
}

export function deactivate() {
    if (client) {
        client.dispose();
    }
}
```

### VSCode 插件: matrixcodeClient.ts

```typescript
// vscode/src/matrixcodeClient.ts

import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';

export interface StreamEvent {
    type: 'text' | 'thinking' | 'tool_use' | 'tool_result' | 'error' | 'done';
    content?: string;
    id?: string;
    name?: string;
    input?: any;
    tool_use_id?: string;
    usage?: { input: number; output: number };
}

export interface RequestContext {
    file?: string;
    language?: string;
    selection?: {
        start: { line: number; character: number };
        end: { line: number; character: number };
    };
}

export class MatrixCodeClient implements vscode.Disposable {
    private process: ChildProcess | null = null;
    private cliPath: string;
    private onEventEmitter = new vscode.EventEmitter<StreamEvent>();
    public readonly onEvent = this.onEventEmitter.event;

    constructor(cliPath: string) {
        this.cliPath = cliPath;
    }

    async checkAvailability(): Promise<boolean> {
        return new Promise((resolve) => {
            const proc = spawn(this.cliPath, ['--version']);
            proc.on('error', () => resolve(false));
            proc.on('exit', (code) => resolve(code === 0));
        });
    }

    async startDaemon(): Promise<void> {
        if (this.process) {
            return;
        }

        return new Promise((resolve, reject) => {
            const config = vscode.workspace.getConfiguration('matrixcode');
            const provider = config.get<string>('provider') || 'anthropic';
            const model = config.get<string>('model') || 'claude-sonnet-4-20250514';

            this.process = spawn(this.cliPath, [
                '--daemon',
                '--provider', provider,
                '--model', model,
                '--json'
            ]);

            this.process.stdout?.on('data', (data) => {
                const lines = data.toString().split('\n').filter(Boolean);
                for (const line of lines) {
                    try {
                        const event: StreamEvent = JSON.parse(line);
                        this.onEventEmitter.fire(event);
                    } catch (e) {
                        console.error('Failed to parse event:', line);
                    }
                }
            });

            this.process.stderr?.on('data', (data) => {
                console.error('MatrixCode stderr:', data.toString());
            });

            this.process.on('error', (err) => {
                vscode.window.showErrorMessage(`MatrixCode error: ${err.message}`);
                reject(err);
            });

            // Wait a bit for daemon to start
            setTimeout(resolve, 500);
        });
    }

    async chat(message: string, context?: RequestContext): Promise<void> {
        const request = {
            type: 'chat',
            content: message,
            context: context ? {
                workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
                file: context.file,
                selection: context.selection
            } : undefined
        };

        this.process?.stdin?.write(JSON.stringify(request) + '\n');
    }

    async newSession(): Promise<void> {
        const request = { type: 'new_session' };
        this.process?.stdin?.write(JSON.stringify(request) + '\n');
    }

    dispose() {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
    }
}
```

## 配置选项

### VSCode 设置

| 设置 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `matrixcode.cliPath` | string | `matrixcode` | CLI 路径 |
| `matrixcode.provider` | enum | `anthropic` | LLM 提供者 |
| `matrixcode.model` | string | `claude-sonnet-4-20250514` | 模型名称 |
| `matrixcode.autoContext` | boolean | `true` | 自动包含文件上下文 |
| `matrixcode.markdown` | boolean | `true` | Markdown 渲染 |
| `matrixcode.think` | boolean | `true` | 扩展思考 |

## 开发优先级

### P0 - MVP (最小可行产品)
1. CLI 添加 `--json` 参数
2. VSCode 侧边栏聊天界面
3. 基本的问答功能

### P1 - 核心功能
1. 代码选区右键操作
2. 文件上下文自动附加
3. Markdown 渲染

### P2 - 增强功能
1. 会话管理
2. 记忆系统 UI
3. 配置界面

### P3 - 高级功能
1. 多文件上下文
2. 诊断信息集成
3. 内联代码建议

## 发布流程

1. **CLI 发布** (现有)
   - crates.io
   - npm
   - GitHub Releases

2. **VSCode 插件发布** (新增)
   - 打包: `vsce package`
   - 发布: `vsce publish`
   - Marketplace: https://marketplace.visualstudio.com/

## 参考项目

- [Claude Code](https://github.com/anthropics/anthropic-quickstarts/tree/main/claude-code) - CLI + VSCode 集成
- [Continue](https://github.com/continuedev/continue) - 开源 AI 编程助手
- [Cline](https://github.com/cline/cline) - VSCode AI 助手