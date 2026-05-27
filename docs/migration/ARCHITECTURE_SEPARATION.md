# MatrixCode 架构分离方案

## 当前架构问题

```
当前: CLI 承担太多职责
┌─────────────────────────────────────┐
│           CLI (混杂)                 │
│  - Agent 核心逻辑                    │
│  - Provider 调用                     │
│  - Tool 执行                         │
│  - Spinner 动画 ❌                   │
│  - 进度显示 ❌                       │
│  - Markdown 渲染 ❌                  │
│  - 终端交互 ❌                       │
└─────────────────────────────────────┘
         ↕ (进程通信，复杂)
┌─────────────────────────────────────┐
│        VSCode 插件                   │
│  - 需要解析 CLI 输出                 │
│  - UI 渲染                           │
└─────────────────────────────────────┘
```

**问题**：
1. CLI 职责不清，既是核心引擎又是 UI
2. UI 代码混在 agent.rs、tools 里
3. VSCode 插件需要复杂的进程通信
4. 测试困难，UI 和逻辑耦合
5. 难以扩展新的 UI（如 Web UI）

---

## 新架构方案

```
新架构: UI 和 CLI 完全分离

┌─────────────────────────────────────┐
│           UI Layer                   │
│  ┌─────────────┐  ┌──────────────┐  │
│  │ VSCode 插件 │  │  终端 UI     │  │
│  │             │  │              │  │
│  │ - 动画      │  │ - spinner    │  │
│  │ - 进度条    │  │ - 进度显示   │  │
│  │ - Tool 卡片 │  │ - markdown   │  │
│  │ - 渲染      │  │ - 颜色/样式  │  │
│  │ - 用户交互  │  │ - 用户输入   │  │
│  └─────────────┘  └──────────────┘  │
│                                     │
│  统一事件处理接口                    │
└─────────────────────────────────────┘
         ↕ (Stream Events / JSON)
┌─────────────────────────────────────┐
│        CLI (Core Agent)              │
│                                     │
│  - Agent 核心逻辑 ✅                 │
│  - Provider 调用 ✅                  │
│  - Tool 执行 ✅                      │
│  - 压缩/记忆 ✅                      │
│  - 任务规划 ✅                       │
│                                     │
│  只输出结构化事件                    │
│  不处理任何 UI ❌                    │
└─────────────────────────────────────┘
```

---

## 核心改动

### 1. 定义结构化事件协议

```rust
// src/event.rs - 新文件
#[derive(Serialize)]
pub struct AgentEvent {
    pub event_type: EventType,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<EventData>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // 流式响应
    TextStart,
    TextDelta,
    TextEnd,
    
    // Thinking
    ThinkingStart,
    ThinkingDelta,
    ThinkingEnd,
    
    // Tool Use
    ToolUseStart,
    ToolUseInputDelta,
    ToolUseEnd,
    ToolResult,
    
    // 状态
    SessionStarted,
    SessionEnded,
    CompressionTriggered,
    Error,
    
    // 元数据
    Usage { input: u64, output: u64 },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventData {
    Text { delta: String },
    Thinking { delta: String, signature: Option<String> },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Error { message: String, code: Option<String> },
    Usage { input_tokens: u64, output_tokens: u64 },
}
```

### 2. CLI Agent 改动

**src/agent.rs** - 移除所有 UI 代码：

```rust
// 改动前
pub async fn run(&mut self, input: String) -> Result<()> {
    // 创建 spinner ❌
    let spinner = Spinner::new("Thinking...");
    
    // 调用 provider
    let response = self.provider.chat(request).await;
    
    // 处理响应，混有 UI 逻辑 ❌
    for event in response.stream {
        match event {
            StreamEvent::Text(text) => {
                spinner.stop();  // ❌ UI 逻辑
                print!("{}", text);  // ❌ 直接打印
            }
        }
    }
}

// 改动后
pub async fn run(&mut self, input: String) -> Result<Vec<AgentEvent>> {
    let mut events = Vec::new();
    
    // 只输出结构化事件
    events.push(AgentEvent::new(EventType::SessionStarted));
    
    // 调用 provider
    let response = self.provider.chat_stream(request).await;
    
    // 处理响应，只产生事件
    for event in response.stream {
        match event {
            StreamEvent::Text(delta) => {
                events.push(AgentEvent::text_delta(delta));
            }
            StreamEvent::ToolUseStart(id, name) => {
                events.push(AgentEvent::tool_use_start(id, name));
            }
        }
    }
    
    events.push(AgentEvent::new(EventType::SessionEnded));
    Ok(events)
}
```

### 3. CLI 主入口改动

**src/main.rs** - 根据模式选择 UI：

```rust
fn main() -> Result<()> {
    let args = parse_args();
    
    // 检查运行模式
    let mode = args.mode.unwrap_or("terminal");
    
    match mode {
        // 终端模式：使用 Terminal UI
        "terminal" | "tui" => {
            run_terminal_ui(args)?;
        }
        
        // 服务模式：只输出 JSON 事件流
        "service" | "json" => {
            run_service_mode(args)?;
        }
        
        // Daemon 模式：供插件调用
        "daemon" => {
            run_daemon_mode(args)?;
        }
    }
}

// 服务模式：纯 JSON 输出
fn run_service_mode(args: Args) -> Result<()> {
    let agent = Agent::new(/* config */);
    
    // 启用 JSON 输出模式
    agent.set_output_format(OutputFormat::JsonStream);
    
    // 运行，输出直接打印到 stdout（JSON 格式）
    let events = agent.run(args.input).await?;
    
    for event in events {
        println!("{}", serde_json::to_string(&event)?);
    }
    
    Ok(())
}

// Daemon 模式：监听 stdin，输出到 stdout
fn run_daemon_mode(args: Args) -> Result<()> {
    let agent = Agent::new(/* config */);
    
    // 监听 stdin 的请求
    let stdin = stdin();
    for line in stdin.lock().lines() {
        let request: DaemonRequest = serde_json::from_str(&line?)?;
        
        // 执行并输出事件流
        let events = agent.handle_request(request).await?;
        
        // 输出 JSON 事件
        for event in events {
            println!("{}", serde_json::to_string(&event)?);
        }
        
        // 输出结束标记
        println!("---END---");
    }
    
    Ok(())
}
```

### 4. 终端 UI 模块

**src/tui/mod.rs** - 新模块：

```rust
// 终端 UI：处理所有显示逻辑
pub struct TerminalUI {
    spinner: Option<Spinner>,
    markdown_renderer: MarkdownRenderer,
    tool_display: ToolDisplay,
}

impl TerminalUI {
    pub fn new() -> Self {
        Self {
            spinner: Spinner::new(),
            markdown_renderer: MarkdownRenderer::new(),
            tool_display: ToolDisplay::new(),
        }
    }
    
    // 处理事件并渲染
    pub fn handle_event(&mut self, event: AgentEvent) {
        match event.event_type {
            EventType::TextStart => {
                self.spinner.stop();
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = event.data {
                    print!("{}", delta);
                }
            }
            EventType::ThinkingStart => {
                self.spinner.set_message("Thinking...");
                self.spinner.start();
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { name, .. }) = event.data {
                    self.spinner.stop();
                    self.tool_display.show_tool_start(name);
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, .. }) = event.data {
                    self.tool_display.show_result(content);
                }
            }
            _ => {}
        }
    }
    
    // 从 JSON 字符串解析并处理
    pub fn handle_json(&mut self, json: &str) -> Result<()> {
        let event: AgentEvent = serde_json::from_str(json)?;
        self.handle_event(event);
        Ok(())
    }
}

// 运行终端 UI
pub fn run_terminal_ui(args: Args) -> Result<()> {
    let agent = Agent::new(/* config */);
    let ui = TerminalUI::new();
    
    // Agent 输出事件
    let events = agent.run(args.input).await?;
    
    // UI 处理事件
    for event in events {
        ui.handle_event(event);
    }
    
    Ok(())
}
```

### 5. VSCode 插件改动

**packages/vscode/src/matrixcodeClient.ts**：

```typescript
// 启动 CLI daemon 模式
export class MatrixCodeClient {
    private process: ChildProcess;
    private eventHandlers: Map<string, (event: AgentEvent) => void>;
    
    async startDaemon(): Promise<void> {
        // 启动 CLI 的 daemon 模式
        this.process = spawn('matrixcode', ['--mode', 'daemon'], {
            stdio: ['pipe', 'pipe', 'pipe']
        });
        
        // 监听 stdout 的 JSON 事件流
        let buffer = '';
        this.process.stdout?.on('data', (data: Buffer) => {
            buffer += data.toString();
            
            // 按行分割，每行是一个 JSON 事件
            const lines = buffer.split('\n');
            buffer = lines.pop() || '';
            
            for (const line of lines) {
                if (line === '---END---') {
                    // 请求完成
                    this.emit('requestComplete');
                } else if (line.trim()) {
                    try {
                        const event: AgentEvent = JSON.parse(line);
                        this.handleEvent(event);
                    } catch (e) {
                        // 解析失败，忽略
                    }
                }
            }
        });
    }
    
    // 处理事件
    private handleEvent(event: AgentEvent): void {
        // 触发对应的处理器
        const handler = this.eventHandlers.get(event.event_type);
        if (handler) {
            handler(event);
        }
        
        // 也触发通用事件
        this.emit('event', event);
    }
    
    // 发送请求
    async chat(content: string, context?: RequestContext): Promise<void> {
        const request = {
            type: 'chat',
            content,
            context
        };
        
        // 写入 stdin
        this.process.stdin?.write(JSON.stringify(request) + '\n');
    }
}
```

**packages/vscode/src/chatView.ts**：

```typescript
// 处理事件并更新 UI
private handleStreamEvent(event: AgentEvent): void {
    switch (event.event_type) {
        case 'text_delta':
            if (event.data?.delta) {
                this.appendToMessage(event.data.delta);
            }
            break;
            
        case 'tool_use_start':
            if (event.data) {
                this.addToolUseCard({
                    id: event.data.id,
                    name: event.data.name,
                    status: 'running'
                });
            }
            break;
            
        case 'tool_result':
            if (event.data) {
                this.updateToolUseCard({
                    tool_use_id: event.data.tool_use_id,
                    result: event.data.content,
                    status: event.data.is_error ? 'error' : 'done'
                });
            }
            break;
            
        case 'thinking_delta':
            if (this.configManager.getShowThinking() && event.data?.delta) {
                this.appendToThinking(event.data.delta);
            }
            break;
    }
}
```

---

## 改动文件清单

### CLI (Rust)

| 文件 | 改动类型 | 说明 |
|------|---------|------|
| src/event.rs | 新增 | 结构化事件定义 |
| src/agent.rs | 重构 | 移除 UI，只产生事件 |
| src/tools/spinner.rs | 移除 | UI 移到 tui 模块 |
| src/ui.rs | 移除 | 移到 tui 模块 |
| src/markdown.rs | 移动 | 移到 tui/markdown.rs |
| src/tui/mod.rs | 新增 | 终端 UI 模块 |
| src/tui/spinner.rs | 新增 | 从 tools/spinner 移来 |
| src/tui/tool_display.rs | 新增 | Tool 显示逻辑 |
| src/main.rs | 重构 | 支持多种模式 |

### VSCode 插件 (TypeScript)

| 文件 | 改动类型 | 说明 |
|------|---------|------|
| src/matrixcodeClient.ts | 重构 | 处理 JSON 事件流 |
| src/chatView.ts | 小改 | 处理结构化事件 |
| src/types.ts | 新增 | AgentEvent 类型定义 |

---

## 优势对比

### 当前架构
```
❌ CLI 混杂 UI 逻辑
❌ 插件需要复杂通信
❌ 测试困难
❌ 难扩展
```

### 新架构
```
✅ CLI 纯粹，只输出事件
✅ UI 完全控制渲染
✅ 通信简单（JSON 流）
✅ 易测试（事件是纯数据）
✅ 易扩展（新 UI 只需处理事件）
```

---

## 实现路线图

### Phase 1: 定义事件协议 (1-2 天)
```rust
// 创建 src/event.rs
// 定义所有事件类型和数据结构
```

### Phase 2: Agent 重构 (2-3 天)
```rust
// 改造 agent.rs
// 移除 UI 代码
// 只输出 AgentEvent
```

### Phase 3: 终端 UI 模块 (2-3 天)
```rust
// 创建 src/tui/
// 移动现有 UI 代码
// 实现事件处理
```

### Phase 4: CLI 模式支持 (1-2 天)
```rust
// 改造 main.rs
// 支持 terminal/service/daemon 模式
```

### Phase 5: VSCode 插件适配 (1-2 天)
```typescript
// 改造 matrixcodeClient.ts
// 使用 daemon 模式
// 处理 JSON 事件流
```

---

## CLI 运行模式

### 1. Terminal 模式（默认）
```bash
matrixcode chat "Fix this error"
# 输出：终端 UI（spinner、颜色、markdown）
```

### 2. Service 模式（纯 JSON）
```bash
matrixcode --mode service chat "Fix this error"
# 输出：JSON 事件流
{"event_type":"session_started","timestamp":123}
{"event_type":"text_delta","data":{"delta":"I'll"}}
{"event_type":"text_delta","data":{"delta":" help"}}
{"event_type":"session_ended","timestamp":456}
```

### 3. Daemon 模式（供插件调用）
```bash
matrixcode --mode daemon
# 监听 stdin，输出到 stdout
# 插件发送请求，CLI 输出事件流
```

---

## 事件流示例

```json
{"event_type":"session_started","timestamp":1000}
{"event_type":"thinking_start","timestamp":1100}
{"event_type":"thinking_delta","data":{"delta":"Analyzing"},"timestamp":1150}
{"event_type":"thinking_end","timestamp":1200}
{"event_type":"tool_use_start","data":{"id":"tool_1","name":"read","input":{"path":"src/main.rs"}},"timestamp":1300}
{"event_type":"tool_use_end","timestamp":1500}
{"event_type":"tool_result","data":{"tool_use_id":"tool_1","content":"fn main() {...}","is_error":false},"timestamp":1600}
{"event_type":"text_start","timestamp":1700}
{"event_type":"text_delta","data":{"delta":"I found"},"timestamp":1750}
{"event_type":"text_delta","data":{"delta":" the issue"},"timestamp":1800}
{"event_type":"text_end","timestamp":1900}
{"event_type":"usage","data":{"input_tokens":500,"output_tokens":200},"timestamp":2000}
{"event_type":"session_ended","timestamp":2100}
```

---

## 下一步行动

1. **先定义事件协议** - 创建 src/event.rs
2. **重构 Agent** - 移除 UI，输出事件
3. **创建 TUI 模块** - 移动现有 UI
4. **支持多模式** - terminal/service/daemon
5. **适配插件** - 使用 daemon 模式

---

## 总结

这个架构改动**非常好**：

✅ **职责清晰** - CLI 专注核心，UI 专注渲染
✅ **易于测试** - Agent 输出纯数据
✅ **易于扩展** - 新 UI 只需处理事件
✅ **通信简单** - JSON 流，无复杂协议
✅ **解耦彻底** - UI 和核心完全分离

建议立即开始实施！