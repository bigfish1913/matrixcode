# 更新日志 (Changelog)

所有重要的更改都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 新增 (Added)

#### 自定义工具注入功能 (v0.4.15)

**核心特性**
- 新增 `ProxyTool` 类型，支持外部系统注册自定义工具
- 新增工具注册中心 `ToolRegistry`，管理代理工具和内置工具
- Agent 支持代理工具 channel，接收外部执行响应
- 新增事件类型 `ProxyToolRequest` 和 `ProxyToolResponse`

**新增文件**
- `core/src/tools/proxy.rs` - ProxyTool 实现 (~160 行)
- `core/src/tools/registry.rs` - 工具注册中心 (~180 行)
- `core/examples/custom_tool_injection.rs` - 完整使用示例
- `docs/CUSTOM_TOOLS.md` - 详细使用文档

**修改文件**
- `core/src/agent/types.rs` - 添加 proxy_tools 和 proxy_rx 字段
- `core/src/agent/run.rs` - 合并代理工具定义，添加 create_proxy_channel()
- `core/src/agent/tools.rs` - execute_tool() 优先识别代理工具
- `core/src/event.rs` - 新增 ProxyToolRequest 和 ProxyToolResponse 事件
- `core/src/tools/mod.rs` - 导出 proxy 和 registry 模块

**架构优势**
- ✅ 零侵入性 - 不修改现有 Tool trait 契约
- ✅ 事件驱动 - 通过事件系统通知调用方
- ✅ 异步等待 - Agent 通过 channel 等待外部执行结果
- ✅ 类型安全 - 使用 ProxyMetadata 传递元数据
- ✅ 优先级机制 - 代理工具优先于内置工具

**使用场景**
- 内部 API 集成（微服务调用）
- 数据库查询（访问内部数据源）
- 第三方服务集成（Slack、邮件等）
- 自定义业务逻辑执行

**执行流程**
```
外部系统注册 ProxyTool
  ↓
Agent 合并工具定义提交给大模型
  ↓
大模型返回 tool_use
  ↓
Agent 识别为代理工具
  ↓
发出 ProxyToolRequest 事件
  ↓
外部系统执行工具逻辑
  ↓
通过 channel 返回 ProxyToolResponse
  ↓
Agent 继续对话流程
```

**API 示例**
```rust
// 创建代理工具
let proxy_tool = ProxyTool::new(
    ToolDefinition {
        name: "custom_search".to_string(),
        description: "搜索内部知识库".to_string(),
        parameters: json!({...}),
    },
    ProxyMetadata {
        tool_type: "search".to_string(),
        endpoint: Some("http://internal-api:8080/search".to_string()),
        timeout_ms: 30000,
        custom: None,
    }
);

// 注册到 Agent
let mut agent = AgentBuilder::new(provider)
    .proxy_tool(proxy_tool)
    .build();

// 监听代理工具事件
loop {
    match agent.next_event().await {
        AgentEvent::ProxyToolRequest { 
            request_id, tool_name, tool_input, metadata, ..
        } => {
            // 外部系统执行
            let result = execute_custom_tool(&tool_name, tool_input).await;
            
            // 返回结果
            proxy_tx.send(ProxyToolResponse {
                request_id,
                result,
                is_error: false,
            }).await?;
        }
        _ => {}
    }
}
```

## [0.4.14] - 2024-01-XX

### 新增 (Added)
- Workflow 自动化系统
- 任务编排引擎
- 规则引擎
- 模板渲染

## [0.4.13] - 2024-01-XX

### 新增 (Added)
- 多 Provider 支持 (OpenAI, Anthropic, Ollama, OpenRouter)
- 流式对话
- 工具调用

## [0.4.0] - 2024-01-XX

### 新增 (Added)
- 初始版本发布
- 核心 Agent 架构
- 基础工具集（read, write, edit, bash 等）
- 会话管理
- 事件系统

---

## 版本说明

- **[Unreleased]**: 开发中的功能
- **[X.Y.Z]**: 正式发布版本

## 变更类型

- **新增 (Added)**: 新功能
- **修改 (Changed)**: 对现有功能的变更
- **弃用 (Deprecated)**: 即将删除的功能
- **移除 (Removed)**: 已删除的功能
- **修复 (Fixed)**: 任何 bug 修复
- **安全 (Security)**: 安全相关的改进