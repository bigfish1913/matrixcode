# 设计方案: MatrixRPC - 动态扩展协议

日期: 2026-06-03

## 核心目标

- 设计自主可控的 JSON-RPC 协议
- 支持第三方用任意语言编写 Tools 和 Workflow 节点
- MatrixCode 作为客户端调用外部服务
- 节点可回调 MatrixCode 的 AI/工具能力
- 与现有 MCP 系统共存，不冲突

## 架构设计

```
                          ┌──────────────────────────────────┐
                          │  External Extension Services     │
                          │  (Python/Go/Rust/...)            │
                          │                                  │
                          │  ┌─────────────────────────────┐ │
                          │  │ Extension Service           │ │
                          │  │ - Tools: [tool1, tool2]     │ │
                          │  │ - Nodes: [node1, node2]     │ │
                          │  │ - Callbacks: [ai, tool]     │ │
                          │  └─────────────────────────────┘ │
                          └──────────────────┬───────────────┘
                                             │
                              JSON-RPC 2.0   │ (Stdio / TCP)
                                             │
                          ┌──────────────────▼───────────────┐
                          │  MatrixCode Extension Gateway    │
                          │                                  │
                          │  ┌─────────────┐ ┌─────────────┐ │
                          │  │ Registry    │ │ Lifecycle   │ │
                          │  │ Service     │ │ Manager     │ │
                          │  │             │ │             │ │
                          │  │ - Register  │ │ - Connect   │ │
                          │  │ - Discover  │ │ - Reconnect │ │
                          │  │ - Unregister│ │ - Disconnect│ │
                          │  └─────────────┘ └─────────────┘ │
                          │                                  │
                          │  ┌─────────────┐ ┌─────────────┐ │
                          │  │ Tool Router │ │ Node Router │ │
                          │  │             │ │             │ │
                          │  │ - Route call│ │ - Route exec│ │
                          │  │ - Validate  │ │ - Callback  │ │
                          │  └─────────────┘ └─────────────┘ │
                          │                                  │
                          │  ┌─────────────────────────────┐ │
                          │  │ Callback Handler            │ │
                          │  │ - AI execution              │ │
                          │  │ - Tool execution            │ │
                          │  │ - Context access            │ │
                          │  └─────────────────────────────┘ │
                          └──────────────────────────────────┘
```

**架构层次：**

1. **Extension Gateway** - 扩展网关，MatrixCode 内的核心组件
2. **Registry Service** - 注册中心，管理外部服务的注册信息
3. **Lifecycle Manager** - 生命周期管理，处理连接、重连、断开
4. **Tool/Node Router** - 调用路由，将请求分发到对应的外部服务
5. **Callback Handler** - 回调处理，处理外部服务的回调请求（AI/工具调用）

**通信模式：**

- **Stdio 模式**：MatrixCode 启动外部进程，通过 stdin/stdout 通信
- **TCP 模式**：外部服务连接 MatrixCode 的 Registry 端口，主动注册

## 数据模型

### Tool Definition（工具定义）

```yaml
ToolDef:
  name: string              # 工具名称，如 "image_analyze"
  description: string       # 工具描述
  parameters: JSONSchema    # 参数 schema
  risk_level: "safe" | "moderate" | "dangerous"  # 风险等级
  timeout_ms: number?       # 超时时间（可选）
```

### Node Definition（节点定义）

```yaml
NodeDef:
  id: string                # 节点 ID
  name: string              # 节点名称
  type: "task" | "condition" | "validate" | "ai" | "composite"
  description: string?      # 节点描述
  capabilities:             # 节点能力声明
    - "ai_execution"        # 可调用 AI
    - "tool_execution"      # 可调用工具
    - "context_access"      # 可访问上下文
  params_schema: JSONSchema # 执行参数 schema
  timeout_ms: number?       # 超时时间
```

### Extension Service（扩展服务）

```yaml
ExtensionService:
  id: string                # 服务 ID（注册时分配）
  name: string              # 服务名称
  version: string           # 版本号
  transport:                # 传输配置
    type: "stdio" | "tcp"
    address: string?        # TCP 地址（仅 TCP 模式）
  tools: ToolDef[]          # 注册的工具列表
  nodes: NodeDef[]          # 注册的节点列表
  status: "connected" | "disconnected" | "reconnecting"
  last_heartbeat: timestamp # 最后心跳时间
  metadata: object?         # 自定义元数据
```

### Registration Request（注册请求）

```yaml
RegisterRequest:
  name: string              # 服务名称
  version: string           # 版本
  transport_type: "stdio" | "tcp"
  tools: ToolDef[]          # 工具定义
  nodes: NodeDef[]          # 节点定义
  metadata: object?         # 自定义元数据
```

### Callback Request（回调请求）

```yaml
CallbackRequest:
  type: "ai" | "tool" | "context"
  request_id: string        # 请求 ID（关联原始调用）
  payload: object           # 回调数据
    # AI 回调: { prompt, context, model_config }
    # Tool 回调: { tool_name, params }
    # Context 回调: { key, operation }
```

**关键实体关系：**

- ExtensionService 包含多个 ToolDef 和 NodeDef
- NodeDef 可声明需要回调的能力（AI、工具、上下文）
- CallbackRequest 由外部服务发起，MatrixCode 处理

## JSON-RPC 接口

### MatrixCode → 外部服务（调用方向）

```json
// 1. 工具调用
{
  "jsonrpc": "2.0",
  "method": "tool.execute",
  "params": {
    "tool_name": "image_analyze",
    "params": { "image_path": "/path/to/image.png" }
  },
  "id": "call-001"
}

// 2. 节点执行
{
  "jsonrpc": "2.0",
  "method": "node.execute",
  "params": {
    "node_id": "validate-node",
    "context": {
      "input_data": {...},
      "variables": {...}
    },
    "callback_endpoint": "matrixcode://callback"
  },
  "id": "call-002"
}

// 3. 心跳检查
{
  "jsonrpc": "2.0",
  "method": "heartbeat",
  "params": {},
  "id": "hb-001"
}
```

### 外部服务 → MatrixCode（注册/回调方向）

```json
// 1. 服务注册（TCP 模式）
{
  "jsonrpc": "2.0",
  "method": "service.register",
  "params": {
    "name": "image-tools",
    "version": "1.0.0",
    "transport_type": "tcp",
    "tools": [
      {
        "name": "image_analyze",
        "description": "分析图片内容",
        "parameters": { "$schema": "..." },
        "risk_level": "safe"
      }
    ],
    "nodes": []
  },
  "id": "reg-001"
}

// 2. 服务注销
{
  "jsonrpc": "2.0",
  "method": "service.unregister",
  "params": { "service_id": "svc-001" },
  "id": "unreg-001"
}

// 3. AI 回调（节点执行时请求 AI 能力）
{
  "jsonrpc": "2.0",
  "method": "callback.ai",
  "params": {
    "request_id": "call-002",
    "prompt": "请分析这个数据...",
    "context": { ... },
    "model_config": { "model": "claude-sonnet" }
  },
  "id": "cb-001"
}

// 4. 工具回调（节点执行时请求工具能力）
{
  "jsonrpc": "2.0",
  "method": "callback.tool",
  "params": {
    "request_id": "call-002",
    "tool_name": "read",
    "params": { "file_path": "/path/to/file" }
  },
  "id": "cb-002"
}

// 5. 上下文回调（节点请求访问工作流上下文）
{
  "jsonrpc": "2.0",
  "method": "callback.context",
  "params": {
    "request_id": "call-002",
    "operation": "get",
    "key": "previous_result"
  },
  "id": "cb-003"
}
```

### 关键接口列表

| 方向 | 方法 | 说明 |
|------|------|------|
| Matrix → External | `tool.execute` | 执行工具 |
| Matrix → External | `node.execute` | 执行节点 |
| Matrix → External | `heartbeat` | 心跳检查 |
| External → Matrix | `service.register` | 注册服务 |
| External → Matrix | `service.unregister` | 注销服务 |
| External → Matrix | `callback.ai` | AI 回调 |
| External → Matrix | `callback.tool` | 工具回调 |
| External → Matrix | `callback.context` | 上下文回调 |

## 传输层设计

### Stdio 模式（本地进程）

```
┌─────────────────────┐                    ┌─────────────────────┐
│  MatrixCode         │                    │  Extension Service  │
│                     │                    │  (子进程)            │
│  ┌───────────────┐  │  stdin ──────────▶ │  ┌───────────────┐  │
│  │ StdioTransport│  │                    │  │ JSON-RPC      │  │
│  │               │◀─│── stdout           │  │ Handler       │  │
│  └───────────────┘  │                    │  └───────────────┘  │
│                     │                    │                     │
└─────────────────────┘                    └─────────────────────┘
```

**Stdio 配置：**

```toml
[extensions.image-tools]
type = "stdio"
command = "python"
args = ["-m", "image_tools_service"]
env = { "API_KEY" = "xxx" }
startup_timeout_ms = 5000
```

**Stdio 特点：**
- MatrixCode 启动子进程，生命周期由 MatrixCode 管理
- 单向通信（MatrixCode → External），无需注册流程
- 服务定义在配置文件中，启动时自动加载

### TCP 模式（远程服务）

```
┌─────────────────────┐                    ┌─────────────────────┐
│  MatrixCode         │                    │  Extension Service  │
│                     │                    │  (远程服务)          │
│  ┌───────────────┐  │  TCP Connection    │  ┌───────────────┐  │
│  │ Registry Port │◀─│──────────────────  │  │ TCP Client    │  │
│  │ (9527)        │  │                    │  │               │  │
│  └───────────────┘  │                    │  └───────────────┘  │
│                     │                    │                     │
│  ┌───────────────┐  │  TCP Callback      │  ┌───────────────┐  │
│  │ Callback Port │◀─│──────────────────  │  │ Callback Send │  │
│  │ (9528)        │  │                    │  │               │  │
│  └───────────────┘  │                    │  └───────────────┘  │
└─────────────────────┘                    └─────────────────────┘
```

**TCP 端口：**
- **Registry Port (9527)**：接受外部服务注册、注销
- **Callback Port (9528)**：接受外部服务的回调请求

**TCP 注册流程：**

```
1. Extension Service 启动
2. 连接 Registry Port (9527)
3. 发送 service.register 请求
4. MatrixCode 返回 service_id
5. 服务进入 connected 状态
6. 定期发送 heartbeat（可选）
```

**TCP 配置：**

```toml
[matrixrpc]
registry_port = 9527
callback_port = 9528
max_connections = 100
heartbeat_interval_ms = 30000
heartbeat_timeout_ms = 10000

[extensions.remote-tools]
type = "tcp"
address = "192.168.1.100:8080"
```

### 消息帧格式

**Stdio 模式：**
```
Content-Length: 123\r\n
\r\n
{"jsonrpc":"2.0",...}
```
（类似 MCP LSP 协议格式）

**TCP 模式：**
```
[4 bytes: length][JSON payload]
```
（二进制帧格式，更高效）

## 生命周期管理

### Stdio 模式生命周期

```rust
pub enum StdioLifecycle {
    Starting,      // MatrixCode 启动子进程
    Initializing,  // 进程启动成功，等待初始化
    Ready,         // 服务就绪，可以调用
    Crashed,       // 进程崩溃或异常退出
    Stopped,       // MatrixCode 主动停止
}
```

**Stdio 重连策略：**
- 进程崩溃后自动重启（最多 3 次）
- 重启间隔：指数退避（1s, 2s, 4s）

### TCP 模式生命周期

```rust
pub enum TcpLifecycle {
    Pending,                    // 外部服务启动，准备连接
    Registering,                // 连接 Registry Port，发送注册请求
    Connected,                  // 注册成功，进入连接状态
    Reconnecting { attempts: u32 }, // 心跳超时，进入重连状态
    Disconnected,               // 主动注销或强制断开
}
```

**TCP 重连策略：**
- 心跳超时后进入 Reconnecting 状态
- 外部服务重新连接 Registry Port
- MatrixCode 根据 service_id 匹配恢复状态
- 最大重连次数：5 次
- 重连间隔：指数退避（2s, 4s, 8s, 16s, 32s）

### 心跳机制（可选）

```json
// MatrixCode → External（心跳检查）
{
  "jsonrpc": "2.0",
  "method": "heartbeat",
  "params": {},
  "id": "hb-001"
}

// External → MatrixCode（心跳响应）
{
  "jsonrpc": "2.0",
  "result": { "status": "ok", "timestamp": 1704067200 },
  "id": "hb-001"
}
```

**心跳配置：**

```toml
[matrixrpc.heartbeat]
interval_ms = 30000    # 心跳间隔
timeout_ms = 10000     # 心跳超时
max_missed = 3         # 最大丢失次数
```

## 错误处理策略

### 错误码定义

遵循 JSON-RPC 2.0 标准，扩展 MatrixRPC 特定错误码：

| 错误码 | 名称 | 说明 |
|--------|------|------|
| -32700 | ParseError | JSON 解析失败 |
| -32600 | InvalidRequest | 无效请求 |
| -32601 | MethodNotFound | 方法不存在 |
| -32602 | InvalidParams | 参数无效 |
| -32603 | InternalError | 内部错误 |
| -32001 | ServiceNotFound | 服务未找到 |
| -32002 | ToolNotFound | 工具未找到 |
| -32003 | NodeNotFound | 节点未找到 |
| -32004 | ExecutionTimeout | 执行超时 |
| -32005 | ServiceDisconnected | 服务已断开 |
| -32006 | CallbackFailed | 回调失败 |
| -32007 | ValidationFailed | 参数验证失败 |
| -32008 | CapabilityNotSupported | 能力不支持 |

### 错误响应格式

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32002,
    "message": "Tool not found",
    "data": {
      "tool_name": "image_analyze",
      "service_id": "svc-001",
      "available_tools": ["other_tool"]
    }
  },
  "id": "call-001"
}
```

### 错误处理策略

**1. 工具/节点执行错误：**
```rust
pub enum ExecutionErrorStrategy {
    ReturnError,                              // 返回错误给调用方
    UseFailureStrategy(FailureStrategy),      // 根据失败策略处理
}
```

**2. 服务断开错误：**
- 立即标记服务为 Disconnected
- 停止路由到该服务的请求
- 尝试重连（如果配置允许）
- 重连成功后恢复路由

**3. 回调失败错误：**
- AI 回调失败：节点执行返回错误
- 工具回调失败：节点可选择替代方案或返回错误
- 上下文回调失败：节点使用默认值或返回错误

**4. 超时处理：**
- 工具/节点执行超时：返回 ExecutionTimeout 错误
- 心跳超时：触发重连流程
- 注册超时：拒绝注册请求

### 重试策略

```rust
pub struct RetryConfig {
    max_attempts: u32,      // 最大重试次数
    interval_ms: u64,       // 重试间隔
    backoff: BackoffStrategy, // 退避策略
}

pub enum BackoffStrategy {
    Fixed,           // 固定间隔
    Linear,          // 线性增长
    Exponential,     // 指数增长
}
```

## 测试策略

### 测试层次

```
┌─────────────────────────────────────────────────────────────┐
│  E2E Tests                                                  │
│  - 完整流程测试（注册 → 执行 → 回调 → 注销）                  │
│  - 多语言 SDK 测试（Python/Go SDK 示例）                     │
│  - 性能测试（并发调用、压力测试）                             │
└─────────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────────┐
│  Integration Tests                                          │
│  - ExtensionGateway + Transport 集成                        │
│  - Registry + LifecycleManager 集成                         │
│  - CallbackHandler + Provider 集成                          │
│  - 真实子进程交互测试                                         │
└─────────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────────┐
│  Unit Tests                                                 │
│  - Registry Service（注册/注销逻辑）                         │
│  - Lifecycle Manager（状态转换）                             │
│  - Tool/Node Router（路由逻辑）                              │
│  - Callback Handler（回调处理）                              │
│  - Transport（消息帧编码/解码）                               │
│  - Error Handler（错误码映射）                               │
└─────────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────────┐
│  Protocol Tests                                             │
│  - JSON-RPC 消息格式验证                                     │
│  - Schema 校验测试                                           │
│  - 兼容性测试（协议版本）                                     │
└─────────────────────────────────────────────────────────────┘
```

### 测试场景

**1. 工具注册与执行：**
```rust
#[test]
async fn test_tool_registration_and_execution() {
    // 1. 启动 mock 服务
    // 2. 注册工具
    // 3. 调用工具
    // 4. 验证结果
}
```

**2. 节点执行与回调：**
```rust
#[test]
async fn test_node_execution_with_callback() {
    // 1. 注册节点（声明 AI 回调能力）
    // 2. 执行节点
    // 3. 节点发起 AI 回调
    // 4. MatrixCode 处理回调
    // 5. 节点继续执行
    // 6. 返回最终结果
}
```

**3. 生命周期测试：**
```rust
#[test]
async fn test_service_reconnect() {
    // 1. 注册服务
    // 2. 模拟断开
    // 3. 服务重连
    // 4. 验证状态恢复
}
```

**4. 错误处理测试：**
```rust
#[test]
async fn test_execution_timeout() {
    // 1. 注册工具（设置超时时间）
    // 2. 工具执行超时
    // 3. 验证返回 ExecutionTimeout 错误
}
```

### Mock 测试工具

```rust
pub struct MockExtensionService {
    tools: Vec<ToolDef>,
    nodes: Vec<NodeDef>,
    response_delay_ms: Option<u64>,
    should_fail: bool,
}

impl MockExtensionService {
    pub fn start_stdio() -> Self;
    pub fn start_tcp(port: u16) -> Self;
    pub fn set_response_delay(delay_ms: u64);
    pub fn set_failure_mode(should_fail: bool);
}
```

### SDK 示例

**Python SDK 示例：**
```python
from matrixrpc import ExtensionService, Tool, Node

service = ExtensionService(name="image-tools")

@service.tool(name="image_analyze")
def analyze_image(image_path: str) -> dict:
    return {"result": "image contains a cat"}

@service.node(id="validate-image")
def validate_node(context: dict, callback) -> dict:
    ai_result = callback.ai("分析这个图片...")
    return {"valid": True}

service.start_tcp(port=8080)
```

## 约束与风险

### 约束

1. 与现有 MCP 系统共存，不产生冲突
2. 使用 JSON-RPC 2.0 标准协议格式
3. 支持 Stdio 和 TCP 两种传输模式
4. 节点回调需要安全验证，防止恶意调用

### 风险及应对

1. **协议兼容性风险**
   - 应对：定义清晰的协议版本号，支持版本协商

2. **安全风险**
   - 应对：TCP 模式支持 TLS 加密，回调需要 token 验证

3. **性能风险**
   - 应对：设计高效的二进制帧格式，支持批量调用

4. **调试难度**
   - 应对：提供详细的日志记录和调试工具

## 验收标准

1. JSON-RPC 协议规范文档完整
2. Extension Gateway 核心组件实现完成
3. Stdio 和 TCP 传输层实现完成
4. 工具注册和执行功能可用
5. 节点注册和执行功能可用
6. AI/工具/上下文回调功能可用
7. 生命周期管理功能可用（注册、重连、注销）
8. Python SDK 示例可用
9. 单元测试覆盖率 > 80%
10. 集成测试通过