# Session & Memory 完整代码路径图

> 所有链接可点击跳转到具体代码位置

---

## 入口点

| 步骤 | 文件 | 函数/位置 | 说明 |
|------|------|-----------|------|
| **① 程序入口** | [main.rs:250](../packages/cli/src/main.rs#L250) | `fn main()` | CLI 启动入口 |
| **② 模式路由** | [main.rs:316-323](../packages/cli/src/main.rs#L316-L323) | `match cli.mode` | 路由到不同模式 |
| **③ TUI 入口** | [main.rs:497](../packages/cli/src/main.rs#L497) | `run_terminal_mode(cli)` | Terminal 模式入口 |

---

## Session 加载链路

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 加载配置** | [main.rs:499](../packages/cli/src/main.rs#L499) | `Config::load()` | 加载 ~/.matrix/config.json |
| **② 创建 SessionManager** | [main.rs:540](../packages/cli/src/main.rs#L540) | `SessionManager::new()` | 创建 session 管理器 |
| **③ 初始化管理器** | [session.rs:491-506](../packages/core/src/session.rs#L491-L506) | `SessionManager::new()` | 加载索引、创建锁 |
| **④ 获取 base_dir** | [session.rs:509-516](../packages/core/src/session.rs#L509-L516) | `get_base_dir()` | 返回 ~/.matrix |
| **⑤ 加载索引** | [session.rs:543-556](../packages/core/src/session.rs#L543-L556) | `load_index()` | 读取 sessions/index.json |
| **⑥ 新建 Session** | [session.rs:580-586](../packages/core/src/session.rs#L580-L586) | `start_new()` | 创建新 session |
| **⑦ 恢复 Session** | [session.rs:601-612](../packages/core/src/session.rs#L601-L612) | `resume()` | 通过 ID/名称恢复 |
| **⑧ 继续 Session** | [session.rs:591-599](../packages/core/src/session.rs#L591-L599) | `continue_last()` | 恢复最近 session |
| **⑨ Session 结构** | [session.rs:265-281](../packages/core/src/session.rs#L265-L281) | `struct Session` | 数据结构定义 |
| **⑩ SessionMetadata** | [session.rs:11-32](../packages/core/src/session.rs#L11-L32) | `struct SessionMetadata` | 元数据结构 |
| **⑪ 文件锁** | [session.rs:360-410](../packages/core/src/session.rs#L360-L410) | `SessionFileLock` | 并发写入保护 |
| **⑫ 保存 Session** | [session.rs:640-665](../packages/core/src/session.rs#L640-L665) | `save_current()` | 持久化到磁盘 |
| **⑬ Session 清理** | [session.rs:853-910](../packages/core/src/session.rs#L853-L910) | `cleanup_old_sessions()` | 清理旧 session |

---

## Memory 加载链路 ★

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 创建 Storage** | [main.rs:668](../packages/cli/src/main.rs#L668) | `MemoryStorage::new()` | 创建记忆存储器 |
| **② Storage 初始化** | [storage.rs:178-186](../packages/core/src/memory/storage.rs#L178-L186) | `MemoryStorage::new()` | 设置路径、创建锁 |
| **③ 加载合并记忆** | [main.rs:669-670](../packages/cli/src/main.rs#L669-L670) | `load_combined()` | 加载全局+项目记忆 |
| **④ load_combined** | [storage.rs:270-285](../packages/core/src/memory/storage.rs#L270-L285) | `load_combined()` | 合并逻辑 ★ |
| **⑤ 加载全局记忆** | [storage.rs:246-253](../packages/core/src/memory/storage.rs#L246-L253) | `load_global()` | ~/.matrix/memory.json |
| **⑥ 加载项目记忆** | [storage.rs:256-266](../packages/core/src/memory/storage.rs#L256-L266) | `load_project()` | {project}/.matrix/memory.json |
| **⑦ AutoMemory 结构** | [types.rs:225-242](../packages/core/src/memory/types.rs#L225-L242) | `struct AutoMemory` | 记忆管理器定义 |
| **⑧ MemoryEntry 结构** | [types.rs:119-144](../packages/core/src/memory/types.rs#L119-L144) | `struct MemoryEntry` | 单条记忆结构 |
| **⑨ MemoryCategory** | [types.rs:39-62](../packages/core/src/memory/types.rs#L39-L62) | `enum MemoryCategory` | 10种记忆分类 |
| **⑩ 文件锁** | [storage.rs:16-61](../packages/core/src/memory/storage.rs#L16-L61) | `MemoryFileLock` | 并发写入保护 |
| **⑪ 发送加载事件** | [main.rs:672-682](../packages/cli/src/main.rs#L672-L682) | `AgentEvent::MemoryLoaded` | TUI 显示记忆条数 |
| **⑫ 生成摘要** | [main.rs:684-687](../packages/cli/src/main.rs#L684-L687) | `generate_prompt_summary()` | 生成初始摘要 |

---

## Memory 摘要生成

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 基础摘要** | [types.rs:931-957](../packages/core/src/memory/types.rs#L931-L957) | `generate_prompt_summary()` | 按重要性排序 |
| **② 上下文摘要** | [types.rs:959-1044](../packages/core/src/memory/types.rs#L959-L1044) | `generate_contextual_summary()` | TF-IDF + 关键词匹配 ★ |
| **③ 关键词提取** | [retrieval.rs:14-62](../packages/core/src/memory/retrieval.rs#L14-L62) | `extract_context_keywords()` | 规则提取 |
| **④ AI 关键词** | [retrieval.rs:187-225](../packages/core/src/memory/retrieval.rs#L187-L225) | `extract_keywords_hybrid()` | AI + 规则混合 |
| **⑤ TF-IDF 搜索** | [retrieval.rs:286-473](../packages/core/src/memory/retrieval.rs#L286-L473) | `TfIdfSearch` | 语义搜索 ★ |
| **⑥ 相关性计算** | [retrieval.rs:106-139](../packages/core/src/memory/retrieval.rs#L106-L139) | `compute_relevance()` | entry 关键词匹配 |
| **⑦ 语义扩展** | [retrieval.rs:74-98](../packages/core/src/memory/retrieval.rs#L74-L98) | `expand_semantic_keywords()` | 别名扩展 |

---

## System Prompt 构建 ★

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 构建入口** | [main.rs:698-704](../packages/cli/src/main.rs#L698-L704) | `build_system_prompt()` | 构建 prompt |
| **② build_system_prompt** | [prompt.rs:442-482](../packages/core/src/prompt.rs#L442-L482) | `build_system_prompt()` | 组合所有部分 ★ |
| **③ 静态 prompt** | [prompt.rs:100-200](../packages/core/src/prompt.rs#L100-L200) | `build_static_system_prompt()` | 身份/思考/约束 |
| **④ 工具 prompt** | [tools/mod.rs](../packages/core/src/tools/mod.rs) | `generate_tools_prompt()` | 工具描述 |
| **⑤ 注入记忆** | [prompt.rs:467-471](../packages/core/src/prompt.rs#L467-L471) | `[ACCUMULATED MEMORY]` | 记忆注入位置 ★ |
| **⑥ 注入项目概述** | [prompt.rs:462-465](../packages/core/src/prompt.rs#L462-L465) | `[PROJECT CONTEXT]` | 项目概述位置 |
| **⑦ 注入 Skills** | [prompt.rs:474-479](../packages/core/src/prompt.rs#L474-L479) | `[AVAILABLE SKILLS]` | 技能列表位置 |

---

## Agent 构建

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① Builder 入口** | [main.rs:706-715](../packages/cli/src/main.rs#L706-L715) | `AgentBuilder::new()` | 创建 builder |
| **② AgentBuilder** | [builder.rs](../packages/core/src/agent/builder.rs) | `struct AgentBuilder` | Builder 结构 |
| **③ 设置 memory** | [builder.rs:92-95](../packages/core/src/agent/builder.rs#L92-L95) | `.memory(summary)` | 设置记忆摘要 |
| **④ 构建 Agent** | [builder.rs](../packages/core/src/agent/builder.rs) | `.build()` | 构建 Agent |
| **⑤ Agent 结构** | [types.rs](../packages/core/src/agent/types.rs) | `struct Agent` | Agent 定义 |
| **⑥ 更新 memory** | [run.rs:83-90](../packages/core/src/agent/run.rs#L83-L90) | `update_memory_summary()` | 动态更新摘要 |
| **⑦ 设置消息** | [run.rs:288-290](../packages/core/src/agent/run.rs#L288-L290) | `set_messages()` | 恢复历史消息 |
| **⑧ 运行循环** | [run.rs:94-285](../packages/core/src/agent/run.rs#L94-L285) | `agent.run()` | 对话主循环 |
| **⑨ 流式处理** | [streaming.rs](../packages/core/src/agent/streaming.rs) | `call_streaming()` | 流式响应处理 |

---

## 首次项目分析

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 触发判断** | [main.rs:738-741](../packages/cli/src/main.rs#L738-L741) | `if !memory_file.exists()` | 首次进入项目 |
| **② 分析入口** | [main.rs:744-747](../packages/cli/src/main.rs#L744-L747) | `generate_project_structure_memories()` | 调用分析 |
| **③ 项目分析** | [project.rs:231-246](../packages/core/src/memory/project.rs#L231-L246) | `generate_project_structure_memories()` | 分析入口 ★ |
| **④ 检测项目类型** | [project.rs:112-133](../packages/core/src/memory/project.rs#L112-L133) | `detect_project_type()` | Cargo.toml/package.json 等 |
| **⑤ 项目配置** | [project.rs:22-79](../packages/core/src/memory/project.rs#L22-L79) | `PROJECT_TYPE_CONFIGS` | 8种项目类型 |
| **⑥ 生成记忆** | [project.rs:190-228](../packages/core/src/memory/project.rs#L190-L228) | `generate_memories()` | 生成结构记忆 |
| **⑦ 保存记忆** | [storage.rs:341-356](../packages/core/src/memory/storage.rs#L341-L356) | `add_entry()` | 添加并保存 |

---

## Memory 动态更新（对话中）

| 步骤 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **① 动态检索** | [main.rs:1518-1545](../packages/cli/src/main.rs#L1518-L1545) | 每轮对话前 | 关键词提取 + 摘要生成 |
| **② 混合关键词** | [main.rs:1521-1527](../packages/cli/src/main.rs#L1521-L1527) | `extract_keywords_hybrid()` | 提取关键词 |
| **③ 上下文摘要** | [main.rs:1530](../packages/cli/src/main.rs#L1530) | `generate_contextual_summary_with_keywords()` | 生成摘要 |
| **④ 更新 Agent** | [main.rs:1534](../packages/cli/src/main.rs#L1534) | `agent.update_memory_summary()` | 更新 prompt |
| **⑤ AI 提取记忆** | [main.rs:1621-1637](../packages/cli/src/main.rs#L1621-L1637) | `detect_memories_smart()` | 对话后提取 ★ |
| **⑥ 提取实现** | [extractor.rs:257-304](../packages/core/src/memory/extractor.rs#L257-L304) | `detect_memories_smart()` | 智能提取 |
| **⑦ AI 提取器** | [extractor.rs:74-111](../packages/core/src/memory/extractor.rs#L74-L111) | `AiMemoryExtractor::extract()` | AI 提取实现 |
| **⑧ 记忆划分** | [main.rs:1644-1656](../packages/cli/src/main.rs#L1644-L1656) | is_project 判断 | 全局 vs 项目 |
| **⑨ 添加记忆** | [types.rs:396-418](../packages/core/src/memory/types.rs#L396-L418) | `AutoMemory::add()` | 重复检查 + 冲突处理 ★ |
| **⑩ 重复检测** | [types.rs:500-537](../packages/core/src/memory/types.rs#L500-L537) | `has_similar()` | Jaccard 相似度 |
| **⑪ 冲突检测** | [types.rs:433-497](../packages/core/src/memory/types.rs#L433-L497) | `find_conflict()` | 矛盾信号检测 |
| **⑫ 修剪** | [types.rs:559-604](../packages/core/src/memory/types.rs#L559-L604) | `prune()` | 移除低重要性 |
| **⑬ 保存记忆** | [storage.rs:341-356](../packages/core/src/memory/storage.rs#L341-L356) | `add_entry()` | 保存到文件 |
| **⑭ 行为推断** | [main.rs:1682-1697](../packages/cli/src/main.rs#L1682-L1697) | `apply_behavior_inferences_to_memory()` | 每5轮推断 |
| **⑮ 定期清理** | [main.rs:1701-1720](../packages/cli/src/main.rs#L1701-L1720) | cleanup | 每10轮清理 |

---

## MemoryEntry 创建方式

| 方式 | 文件位置 | 函数 | 说明 |
|------|----------|------|------|
| **基本创建** | [types.rs:148-163](../packages/core/src/memory/types.rs#L148-L163) | `MemoryEntry::new()` | 带所有参数 |
| **手动创建** | [types.rs:166-176](../packages/core/src/memory/types.rs#L166-L176) | `MemoryEntry::manual()` | 带 project_path |
| **全局手动** | [types.rs:172-176](../packages/core/src/memory/types.rs#L172-L176) | `MemoryEntry::manual_global()` | 无 project_path |
| **AI 提取** | [extractor.rs:165](../packages/core/src/memory/extractor.rs#L165) | `MemoryEntry::new()` | AI 提取创建 |
| **项目分析** | [project.rs:199-225](../packages/core/src/memory/project.rs#L199-L225) | `MemoryEntry::new()` | 项目结构记忆 |
| **行为推断** | [learning.rs:315](../packages/core/src/memory/learning.rs#L315) | `MemoryEntry::new()` | 用户偏好推断 |
| **合并创建** | [types.rs:713](../packages/core/src/memory/types.rs#L713) | `MemoryEntry::new()` | 合并相似记忆 |

---

## Core 模块导出

| 模块 | 文件位置 | 导出内容 |
|------|----------|----------|
| **lib.rs** | [lib.rs](../packages/core/src/lib.rs) | 所有公开模块导出 |
| **Session** | [lib.rs:35](../packages/core/src/lib.rs#L35) | `pub use session::{Session, SessionManager}` |
| **Memory** | [lib.rs:13](../packages/core/src/lib.rs#L13) | `pub mod memory` |
| **Agent** | [lib.rs:26](../packages/core/src/lib.rs#L26) | `pub use agent::{Agent, AgentBuilder}` |
| **Memory mod** | [memory/mod.rs:27-35](../packages/core/src/memory/mod.rs#L27-L35) | re-export 所有子模块 |

---

## 存储文件路径

| 类型 | 路径 | 相关代码 |
|------|------|----------|
| **全局记忆** | `~/.matrix/memory.json` | [storage.rs:206-208](../packages/core/src/memory/storage.rs#L206-L208) |
| **项目记忆** | `{project}/.matrix/memory.json` | [storage.rs:211-215](../packages/core/src/memory/storage.rs#L211-L215) |
| **Session 索引** | `~/.matrix/sessions/index.json` | [session.rs:524-526](../packages/core/src/session.rs#L524-L526) |
| **Session 文件** | `~/.matrix/sessions/{id}.json` | [session.rs:529-531](../packages/core/src/session.rs#L529-L531) |
| **配置文件** | `~/.matrix/config.json` | [config.rs](../packages/core/src/config.rs) |
| **项目概述** | `{project}/MATRIX.md` | [overview.rs](../packages/core/src/overview.rs) |

---

## 精简版：关键路径一键跳转

### Session 加载核心
```
[SessionManager::new()](../packages/core/src/session.rs#L491)
  → [load_index()](../packages/core/src/session.rs#L543)
  → [start_new()](../packages/core/src/session.rs#L580) / [resume()](../packages/core/src/session.rs#L601)
```

### Memory 加载核心
```
[MemoryStorage::new()](../packages/core/src/memory/storage.rs#L178)
  → [load_combined()](../packages/core/src/memory/storage.rs#L270)
  → [load_global()](../packages/core/src/memory/storage.rs#L246) + [load_project()](../packages/core/src/memory/storage.rs#L256)
```

### Memory 注入 Prompt
```
[generate_prompt_summary()](../packages/core/src/memory/types.rs#L931)
  → [build_system_prompt()](../packages/core/src/prompt.rs#L442)
  → `[ACCUMULATED MEMORY]` 注入 [prompt.rs:467-471](../packages/core/src/prompt.rs#L467-L471)
```

### Memory 动态提取
```
[detect_memories_smart()](../packages/core/src/memory/extractor.rs#L257)
  → [AiMemoryExtractor::extract()](../packages/core/src/memory/extractor.rs#L74)
  → [AutoMemory::add()](../packages/core/src/memory/types.rs#L396)
  → [add_entry()](../packages/core/src/memory/storage.rs#L341)
```

### Agent 运行循环
```
[agent.run()](../packages/core/src/agent/run.rs#L94)
  → [call_streaming()](../packages/core/src/agent/streaming.rs)
  → [process_response()](../packages/core/src/agent/run.rs)
```

---

## 版本信息

- 文档生成时间：2026-05-24
- 适用版本：MatrixCode v0.4.13+
- 相关文档：[SESSION_MEMORY_ARCHITECTURE.md](./SESSION_MEMORY_ARCHITECTURE.md)