# 功能迁移对比分析

## 一、模块文件对比

| 模块 | 旧版本 | 新版本 | 状态 |
|------|--------|--------|------|
| agent.rs | ✅ | ✅ 简化版 | ⚠️ 功能缺失 |
| main.rs | ✅ | ✅ CLI简化版 | ⚠️ 功能缺失 |
| models.rs | 614行 | 614行 | ✅ 完全迁移 |
| memory.rs | 4195行 | 4212行 | ✅ 完全迁移 |
| compress.rs | 1100行 | 1100行 | ✅ 完全迁移 |
| providers/ | ✅ | ✅ | ✅ 完全迁移 |
| tools/ | ✅ | ✅ | ✅ 完全迁移 |
| session.rs | ✅ | ✅ | ✅ 完全迁移 |
| skills.rs | ✅ | ✅ | ✅ 完全迁移 |
| config.rs | ✅ | ✅ 简化版 | ⚠️ 功能缺失 |
| ipc.rs | ✅ | ❌ 删除 | → event.rs |
| markdown.rs | ✅ | ✅ TUI | ✅ 已迁移 |
| ui.rs | ✅ | ✅ TUI | ✅ 已迁移 |

---

## 二、Agent 功能对比

### 旧 agent.rs 有，新 agent.rs 缺少

| 功能 | 说明 | 优先级 |
|------|------|--------|
| `profile()` | PromptProfile 配置 | P1 |
| `skills()` | 技能加载 | P1 |
| `overview()` | 项目概览注入 | P2 |
| `memory()` | 记忆摘要注入 | P2 |
| `quiet()` | daemon 静默模式 | P3 |
| `markdown()` | markdown 渲染开关 | P3 |
| `compress_provider` | 压缩模型 | P2 |
| `plan_provider` | 规划模型 | P2 |
| `streaming` | 流式响应处理 | P1 |

### 新 agent.rs 已有

| 功能 | 说明 |
|------|------|
| `run()` | 主循环 + tool执行 |
| `event_sender()` | 事件通道 |
| `set_cancel_token()` | 取消支持 |
| `process_response()` | 响应处理 |
| `execute_tool()` | 工具执行 |
| `track_usage()` | Token跟踪 |
| `emit()` | 事件发送 |

---

## 三、CLI main.rs 功能对比

### 旧 main.rs 有，新 CLI 缺少

| 功能 | 说明 | 优先级 |
|------|------|--------|
| `run_repl()` | REPL交互循环 | P1 |
| `load_skills()` | 加载技能 | P1 |
| `show_session_picker()` | 会话选择UI | P2 |
| `list_sessions()` | 列出会话 | P2 |
| `print_status()` | 显示状态 | P2 |
| `print_history()` | 显示历史 | P3 |
| `handle_init()` | 初始化项目 | P3 |
| `handle_overview()` | 项目概览 | P3 |
| `handle_compress()` | 手动压缩 | P3 |
| `handle_plan()` | 任务规划 | P2 |
| `handle_models()` | 模型管理 | P2 |

### 新 CLI 已有

| 功能 | 说明 |
|------|------|
| `run_terminal_mode()` | 终端模式 |
| `run_service_mode()` | JSON模式 |
| `run_daemon_mode()` | daemon模式 |
| `handle_daemon_request()` | daemon请求处理 |

---

## 四、Config 功能对比

### 旧 config.rs 有，新缺少

| 功能 | 说明 |
|------|------|
| Provider配置 | Anthropic/OpenAI |
| 多模型配置 | main/plan/compress/fast |
| 环境变量加载 | .env支持 |
| CLI参数解析 | clap集成 |

### 新 config.rs 状态

简化版，只有基本配置：
- system_prompt
- max_tokens
- think
- approve_mode
- model

---

## 五、建议迁移顺序

### P1 - 必需功能

1. **Agent 技能加载** - skills()
2. **Agent 流式处理** - 真正的streaming API调用
3. **CLI REPL循环** - 交互式聊天
4. **CLI 技能加载** - load_skills()

### P2 - 重要功能

5. **Agent profile** - PromptProfile
6. **Agent overview/memory** - 项目概览和记忆
7. **CLI 会话管理** - list_sessions/show_picker
8. **CLI 规划功能** - handle_plan()
9. **Config 完整版** - 多模型/环境变量

### P3 - 可选功能

10. **Agent markdown开关**
11. **CLI 状态显示**
12. **CLI 手动压缩**
13. **CLI 初始化**

---

## 六、当前可用功能

### ✅ 已可用

- Core providers (Anthropic/OpenAI)
- Core tools (13个工具)
- Core session/memory/compress/models
- TUI ui/markdown
- CLI daemon模式 (JSON输出)
- VSCode 插件 (daemon通信)

### ⚠️ 简化版需完善

- Agent (缺少技能/流式/profile)
- CLI (缺少REPL/会话管理)
- Config (缺少完整配置)

---

## 七、下一步建议

1. **完善 Agent** - 添加 skills/profile/streaming
2. **完善 CLI** - 添加 REPL 循环
3. **完善 Config** - 添加完整配置加载

完成后可删除 `_src_old/` 目录。