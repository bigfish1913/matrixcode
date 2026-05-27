# 自定义工具注入 (Custom Tool Injection)

## 📖 概述

MatrixCode 支持自定义工具注入功能，允许外部系统注册自己的工具，由调用方执行而非 Agent 内部执行。这为集成第三方服务、调用内部 API、执行特定业务逻辑提供了灵活的扩展机制。

## 🎯 核心特性

- ✅ **零侵入性** - 不修改现有 Tool trait 契约
- ✅ **事件驱动** - 通过事件系统通知调用方
- ✅ **异步等待** - Agent 通过 channel 等待外部执行结果
- ✅ **类型安全** - 使用 ProxyMetadata 传递元数据
- ✅ **优先级机制** - 代理工具优先于内置工具

---

## 🚀 快速开始

### 1. 创建代理工具

```rust
use matrixcode_core::tools::{ProxyTool, ToolDefinition};
use serde_json::json;

let proxy_tool = ProxyTool::new(
    ToolDefinition {
        name: "custom_search".to_string(),
        description: "搜索内部知识库".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                },
                "limit": {
                    "type": "integer",
                    "description": "返回结果数量",
                    "default": 10
                }
            },
            "required": ["query"]
        }),
    },
    ProxyMetadata {
        tool_type: "search".to_string(),
        endpoint: "http://internal-api:8080/search".to_string(),
        timeout_ms: 30000,
        custom: Some(json!({
            "auth_required": true,
            "cache_enabled": true
        })),
    }
);
```

### 2. 注册到 Agent

```rust
use matrixcode_core::agent::AgentBuilder;

let mut agent = AgentBuilder::new(provider)
    .system_prompt("你是一个助手，可以搜索内部知识库。")
    .proxy_tool(proxy_tool)  // 注册代理工具
    .build();
```

### 3. 创建响应 Channel

```rust
// 创建用于发送响应的 channel
let proxy_tx = agent.create_proxy_channel();
```

### 4. 监听并处理事件

```rust
use matrixcode_core::event::AgentEvent;

loop {
    match agent.next_event().await {
        AgentEvent::ProxyToolRequest { 
            request_id, 
            tool_name, 
            tool_input,
            metadata,
            ..
        } => {
            println!("收到代理工具请求: {}", tool_name);
            println!("输入参数: {}", tool_input);
            println!("元数据: {:?}", metadata);
            
            // 调用方自己执行工具逻辑
            let result = execute_custom_tool(&tool_name, tool_input).await;
            
            // 返回结果给 Agent
            proxy_tx.send(ProxyToolResponse {
                request_id,
                result: result.content,
                is_error: result.is_error,
            }).await?;
        }
        
        AgentEvent::Text { content } => {
            print!("{}", content);
        }
        
        AgentEvent::ToolUse { name, input } => {
            println!("内置工具调用: {}({})", name, input);
        }
        
        AgentEvent::Complete => break,
        
        _ => {}
    }
}
```

---

## 📐 架构设计

### 执行流程

```
┌─────────────────────────────────────────────────────────────┐
│                     外部系统 (调用方)                          │
│  ┌──────────────┐                     ┌──────────────────┐  │
│  │  ProxyTool   │ ─────(注册)────────>│      Agent       │  │
│  │  定义+元数据   │                     │                  │  │
│  └──────────────┘                     └──────────────────┘  │
│         │                                      │            │
│         │                                      │            │
│         │                              ┌──────▼──────────┐   │
│         │                              │   大模型 (LLM)   │   │
│         │                              │  选择工具调用    │   │
│         │                              └──────┬──────────┘   │
│         │                                     │              │
│         │                         ┌───────────▼────────────┐ │
│         │                         │ Agent 识别代理工具      │ │
│         │                         │ 发出 ProxyToolRequest  │ │
│         │                         └───────────┬────────────┘ │
│         │                                     │              │
│         │              ┌──────────────────────▼─────────────┐│
│         │              │  AgentEvent::ProxyToolRequest      ││
│         │              │  { request_id, tool_input, ... }  ││
│         │              └──────────────────────┬─────────────┘│
│         │                                     │              │
│         ▼                                     │              │
│  ┌────────────────┐                          │              │
│  │ 外部执行工具    │◄─────────────────────────┘              │
│  │ (HTTP调用/本地)  │                                        │
│  └────────┬───────┘                                         │
│           │                                                 │
│           │  执行结果                                        │
│           ▼                                                 │
│  ┌────────────────────┐                                     │
│  │ ProxyToolResponse  │──────────> Agent 继续对话流程         │
│  │ { request_id, ... }│                                     │
│  └────────────────────┘                                     │
└─────────────────────────────────────────────────────────────┘
```

### 核心类型

#### ProxyTool

```rust
pub struct ProxyTool {
    definition: ToolDefinition,    // 工具定义（提交给大模型）
    metadata: ProxyMetadata,       // 元数据（传递给调用方）
}

pub struct ProxyMetadata {
    pub tool_type: String,         // 工具类型标识
    pub endpoint: String,           // 执行端点 URL
    pub timeout_ms: u64,           // 超时时间
    pub custom: Option<Value>,     // 自定义元数据
}
```

#### AgentEvent

```rust
pub enum AgentEvent {
    // 代理工具请求事件
    ProxyToolRequest {
        request_id: String,        // 请求唯一 ID
        tool_name: String,         // 工具名称
        tool_input: Value,         // 输入参数
        metadata: ProxyMetadata,   // 工具元数据
    },
    
    // 代理工具响应（Agent 内部使用）
    ProxyToolResponse {
        request_id: String,
        result: String,
        is_error: bool,
    },
    
    // ... 其他事件类型
}
```

---

## 🔧 高级用法

### 1. 多工具注册

```rust
let agent = AgentBuilder::new(provider)
    .proxy_tool(search_tool)
    .proxy_tool(database_tool)
    .proxy_tool(api_tool)
    .build();
```

### 2. 动态工具注入

```rust
// 运行时动态添加代理工具
agent.add_proxy_tool(new_tool).await?;
```

### 3. 工具优先级

代理工具优先级高于内置工具：

```rust
// 如果代理工具和内置工具同名，优先使用代理工具
let proxy_tool = ProxyTool::new(
    ToolDefinition { name: "read_file".to_string(), ... },
    metadata,
);

// Agent 会优先调用代理工具，而非内置的 read_file
```

### 4. 超时控制

```rust
// 在外部执行时处理超时
let result = tokio::time::timeout(
    Duration::from_millis(metadata.timeout_ms),
    execute_custom_tool(&tool_name, tool_input)
).await.unwrap_or_else(|_| {
    Err("Tool execution timeout".to_string())
});
```

### 5. 错误处理

```rust
match execute_custom_tool(&tool_name, tool_input).await {
    Ok(result) => {
        proxy_tx.send(ProxyToolResponse {
            request_id,
            result,
            is_error: false,
        }).await?;
    }
    Err(e) => {
        proxy_tx.send(ProxyToolResponse {
            request_id,
            result: format!("Error: {}", e),
            is_error: true,
        }).await?;
    }
}
```

---

## 💡 使用场景

### 1. 内部 API 集成

```rust
// 调用内部微服务
let proxy_tool = ProxyTool::new(
    ToolDefinition {
        name: "query_user_info".to_string(),
        description: "查询用户信息".to_string(),
        parameters: json!({...}),
    },
    ProxyMetadata {
        tool_type: "http".to_string(),
        endpoint: "http://user-service:8080/api/user".to_string(),
        timeout_ms: 5000,
        custom: Some(json!({"auth": "internal-token"})),
    },
);
```

### 2. 数据库查询

```rust
// 执行数据库查询
let proxy_tool = ProxyTool::new(
    ToolDefinition {
        name: "query_database".to_string(),
        description: "查询内部数据库".to_string(),
        parameters: json!({...}),
    },
    ProxyMetadata {
        tool_type: "database".to_string(),
        endpoint: "postgresql://internal-db:5432".to_string(),
        timeout_ms: 10000,
        custom: Some(json!({"max_rows": 1000})),
    },
);
```

### 3. 第三方服务集成

```rust
// 调用第三方 API
let proxy_tool = ProxyTool::new(
    ToolDefinition {
        name: "send_slack_message".to_string(),
        description: "发送 Slack 消息".to_string(),
        parameters: json!({...}),
    },
    ProxyMetadata {
        tool_type: "slack".to_string(),
        endpoint: "https://slack.com/api/chat.postMessage".to_string(),
        timeout_ms: 3000,
        custom: Some(json!({"channel": "#general"})),
    },
);
```

### 4. 自定义业务逻辑

```rust
// 执行特定业务逻辑
async fn execute_custom_tool(name: &str, input: Value) -> Result<String> {
    match name {
        "calculate_risk" => {
            let score = calculate_risk_score(input["user_id"].as_str().unwrap());
            Ok(json!({"risk_score": score}).to_string())
        }
        "check_compliance" => {
            let result = check_compliance_rules(input).await;
            Ok(result.to_string())
        }
        _ => Err(format!("Unknown tool: {}", name))
    }
}
```

---

## 📊 性能优化建议

### 1. 并发执行

```rust
// 使用 tokio::spawn 并发处理多个工具请求
tokio::spawn(async move {
    while let Some(request) = request_rx.recv().await {
        let tx = proxy_tx.clone();
        tokio::spawn(async move {
            let result = execute_tool(request).await;
            tx.send(result).await;
        });
    }
});
```

### 2. 缓存机制

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct ToolCache {
    cache: HashMap<String, (String, Instant)>,
    ttl: Duration,
}

impl ToolCache {
    fn get(&self, key: &str) -> Option<&String> {
        self.cache.get(key)
            .filter(|(_, time)| time.elapsed() < self.ttl)
            .map(|(result, _)| result)
    }
}
```

### 3. 连接池

```rust
// 使用 reqwest 连接池
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(30))
    .build()?;
```

---

## 🔒 安全考虑

### 1. 参数验证

```rust
// 在外部执行前验证参数
fn validate_input(tool_name: &str, input: &Value) -> Result<()> {
    match tool_name {
        "query_database" => {
            if input.get("limit").and_then(|v| v.as_u64()).unwrap_or(0) > 10000 {
                return Err("Limit too high".into());
            }
        }
        _ => {}
    }
    Ok(())
}
```

### 2. 权限控制

```rust
// 检查调用权限
fn check_permission(user: &User, tool_name: &str) -> Result<()> {
    if !user.can_use_tool(tool_name) {
        return Err("Permission denied".into());
    }
    Ok(())
}
```

### 3. 敏感信息保护

```rust
// 不在日志中记录敏感参数
fn log_tool_call(tool_name: &str, input: &Value) {
    let sanitized = sanitize_sensitive_fields(input);
    info!("Tool call: {}({})", tool_name, sanitized);
}
```

---

## 📚 完整示例

查看 `examples/custom_tool_injection.rs` 获取完整可运行示例。

---

## 🔗 相关文档

- [Agent API 文档](./AGENT_API.md)
- [Tool 系统](./TOOLS.md)
- [事件系统](./EVENTS.md)
- [API 参考](https://docs.rs/matrixcode-core)

---

## ❓ 常见问题

### Q: 代理工具和内置工具的区别？

**A:** 内置工具由 Agent 内部执行，代理工具由外部系统执行。代理工具适用于：
- 需要访问外部资源（HTTP API、数据库等）
- 需要特殊权限或认证
- 业务逻辑不适合放在 Agent 内部

### Q: 如何调试代理工具？

**A:** 可以通过以下方式调试：
1. 启用 DEBUG 日志：`RUST_LOG=debug`
2. 监听 `AgentEvent::ProxyToolRequest` 事件
3. 记录请求和响应数据

### Q: 代理工具超时怎么办？

**A:** 
1. 设置合理的 `timeout_ms`
2. 在外部执行时使用 `tokio::time::timeout`
3. 返回错误信息让 Agent 处理

### Q: 如何实现工具链（Tool Chaining）？

**A:** Agent 会自动处理工具链，你只需要：
1. 返回正确格式的结果
2. Agent 会继续调用大模型，直到任务完成

---

## 📝 更新日志

- **v0.4.15** - 初始实现自定义工具注入功能
- 支持代理工具注册、事件通知、异步等待
- 添加完整文档和示例代码

---

如有问题或建议，欢迎提交 Issue 或 PR！