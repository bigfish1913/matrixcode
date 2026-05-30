# 待办事项

## 动态扩展能力

### 1. 动态接入 Workflow 节点

**目标**: 支持通过 JSON-RPC 协议动态接入外部 workflow 节点

**要点**:
- [ ] 设计 JSON-RPC 接口规范
- [ ] 实现节点注册与发现机制
- [ ] 支持节点生命周期管理（连接、断开、重连）
- [ ] 定义节点间通信协议
- [ ] 实现节点健康检查

---

### 2. 动态 Tools 接入

**目标**: 支持通过 JSON-RPC 协议动态注册和使用外部工具

**要点**:
- [ ] 设计 Tool JSON-RPC 接口规范
- [ ] 实现工具动态注册机制
- [ ] 支持工具参数校验
- [ ] 实现工具调用路由
- [ ] 支持工具结果缓存

---

## 消息队列优化

### 3. 实时追加消息（Agent 重构）

**目标**: 在 AI 处理过程中支持追加消息，而不是等一轮结束

**状态**: ✅ 已完成

**实现内容**:
- [x] 增加 TASK_CHANNEL_BUFFER 到 100
- [x] 合并队列消息为一个请求（备选方案保留）
- [x] Agent 支持实时追加消息（核心重构）
  - [x] 添加 `pending_input_rx` channel 到 Agent
  - [x] 在 `call_streaming` 中使用 `select!` 监听追加消息
  - [x] 追加消息处理：缓存并在当前轮完成后处理
- [x] TUI 实时推送队列消息到 Agent

---

## 技术方案

### JSON-RPC 通信架构

```
┌─────────────────┐     JSON-RPC     ┌─────────────────┐
│   MatrixCode    │ ◄──────────────► │  External Node  │
│     Core        │                  │   / Tool        │
└─────────────────┘                  └─────────────────┘
```

### 实时追加消息架构

```
用户输入 ──► pending_messages (TUI)
                 │
                 ▼ (实时推送)
            pending_input_rx (Agent)
                 │
                 ▼ (select! 监听)
         ┌──────┴──────┐
         │             │
    API 流式响应   新消息到达
         │             │
         ▼             ▼
    继续处理       缓存/追加
```

### 相关参考

- [MCP Guide](packages/docs/mcp-guide.md) - 现有 MCP 工具集成方案
- [Custom Tools](packages/docs/CUSTOM_TOOLS.md) - 自定义工具文档


## 问题 

1. 避免使用全局搜索文件