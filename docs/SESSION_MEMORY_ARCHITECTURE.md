# Session & Memory 系统架构文档

## 目录

1. [概述](#概述)
2. [Session 系统](#session-系统)
   - [数据结构](#session-数据结构)
   - [生命周期](#session-生命周期)
   - [存储机制](#session-存储机制)
   - [文件锁机制](#session-文件锁机制)
3. [Memory 系统](#memory-系统)
   - [数据结构](#memory-数据结构)
   - [记忆分类](#记忆分类)
   - [加载流程](#memory-加载流程)
   - [记忆提取](#记忆提取)
   - [存储机制](#memory-存储机制)
   - [检索机制](#检索机制)
4. [完整代码路径](#完整代码路径)
5. [数据流向图](#数据流向图)
6. [关键时机总结](#关键时机总结)

---

## 概述

MatrixCode 的 Session 和 Memory 系统提供了跨会话的持久化能力：

- **Session**: 保存对话历史、压缩消息、Token 统计等，支持 `--continue` 和 `--resume` 恢复
- **Memory**: 自动积累用户偏好、项目决策、技术发现等，在 system prompt 中注入上下文

两个系统都采用：
- JSON 文件存储
- 文件锁防止并发写入
- 原子写入（tmp → rename）保证数据安全

---

## Session 系统

### Session 数据结构

#### SessionMetadata（元数据）

位置：`packages/core/src/session.rs:11-32`

```rust
pub struct SessionMetadata {
    pub id: String,                          // UUID 唯一标识
    pub name: Option<String>,                // 用户定义名称（或自动生成）
    pub project_path: Option<String>,        // 项目路径关联
    pub created_at: DateTime<Utc>,           // 创建时间
    pub updated_at: DateTime<Utc>,           // 最后更新时间
    pub message_count: usize,                // 消息数量
    pub last_input_tokens: u64,              // 上次输入 tokens
    pub total_output_tokens: u64,            // 累计输出 tokens
    pub compression_history: Vec<CompressionHistoryEntry>, // 压缩历史
}
```

#### Session（完整数据）

位置：`packages/core/src/session.rs:265-281`

```rust
pub struct Session {
    pub metadata: SessionMetadata,
    pub full_messages: Vec<Message>,         // TUI 显示用的完整消息
    pub compressed_messages: Vec<Message>,   // API 请求用的压缩消息
    pub message_summaries: Vec<MessageSummary>, // 压缩消息摘要
}
```

#### SessionIndex（索引）

位置：`packages/core/src/session.rs:129-135`

```rust
pub struct SessionIndex {
    pub sessions: Vec<SessionMetadata>,      // 所有已知 session
    pub last_session_id: Option<String>,     // 最近活跃 session（--continue 用）
}
```

### Session 生命周期

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Session 生命周期                                      │
└─────────────────────────────────────────────────────────────────────────────┘

1. 启动阶段
   ┌──────────────────┐
   │ CLI 启动         │
   │ matrixcode       │
   └─────────┬────────┘
             │
             ▼
   ┌──────────────────┐
   │SessionManager::new│──▶ 加载 ~/.matrix/sessions/index.json
   └─────────┬────────┘
             │
             ▼
   ┌──────────────────────────────────────────┐
   │ 判断模式:                                 │
   │  • --continue: continue_last()           │
   │  • --resume ID: resume(query)            │
   │  • 新会话: start_new(project_path)       │
   └─────────┬────────────────────────────────┘
             │
             ▼
   ┌──────────────────┐
   │ Session::new()   │──▶ UUID 生成 + 时间戳
   └─────────┬────────┘
             │
             ▼
   ┌──────────────────┐
   │ save_current()   │──▶ 写入 ~/.matrix/sessions/{id}.json
   └──────────────────┘

2. 对话阶段
   ┌──────────────────┐
   │ 用户输入消息     │
   └─────────┬────────┘
             │
             ▼
   ┌──────────────────┐
   │ agent.run(msg)   │──▶ messages.push(user_msg)
   └─────────┬────────┘──▶ API 调用 + 工具执行
             │
             ▼
   ┌──────────────────────────────────────────────┐
   │ 每轮对话结束自动保存:                         │
   │  1. set_messages(messages)                   │
   │  2. set_compressed_messages(messages)        │
   │  3. update_stats(tokens)                     │
   │  4. save_current()                           │
   └──────────────────────────────────────────────┘

3. 恢复阶段
   ┌──────────────────┐
   │ --continue 或    │
   │ --resume ID      │
   └─────────┬────────┘
             │
             ▼
   ┌──────────────────┐
   │ load_session(id) │──▶ 读取 {id}.json
   └─────────┬────────┘──▶ migrate_legacy()
             │           ──▶ 恢复 project_path
             ▼
   ┌──────────────────┐
   │ 返回消息给 Agent │
   │ full → TUI 显示  │
   │ api → Agent API  │
   └──────────────────┘
```

### Session 存储机制

**存储路径**：
- 索引文件：`~/.matrix/sessions/index.json`
- Session 文件：`~/.matrix/sessions/{id}.json`

**原子写入流程**（`save_current()`）：

位置：`packages/core/src/session.rs:640-665`

```rust
pub fn save_current(&mut self) -> Result<()> {
    // 1. 获取文件锁（5秒超时）
    self.lock.acquire(5000)?;

    // 2. 更新索引
    self.index.upsert(session_clone.metadata.clone());
    self.save_index_locked()?;

    // 3. 写入 session 文件（原子操作）
    let path = self.session_path(&session_clone.metadata.id);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;  // 原子替换

    // 4. 释放文件锁
    self.lock.release()?;
}
```

### Session 文件锁机制

位置：`packages/core/src/session.rs:360-477`

特性：
- 阻塞式获取锁，超时 5 秒
- 检测 stale lock（进程死亡或锁过期 > 60秒）
- 跨平台支持（Unix `/proc`，Windows `tasklist`）
- `Drop` trait 自动释放锁

---

## Memory 系统

### Memory 数据结构

#### MemoryEntry（单条记忆）

位置：`packages/core/src/memory/types.rs:119-144`

```rust
pub struct MemoryEntry {
    pub id: String,                          // UUID
    pub created_at: DateTime<Utc>,           // 创建时间
    pub last_referenced: DateTime<Utc>,      // 最后引用时间
    pub category: MemoryCategory,            // 分类
    pub content: String,                     // 记忆内容
    pub source_session: Option<String>,      // 来源 session ID
    pub project_path: Option<String>,        // 项目路径
    pub reference_count: u32,                // 引用次数
    pub importance: f64,                     // 重要性分数 (0-100)
    pub tags: Vec<String>,                   // 搜索标签
    pub is_manual: bool,                     // 是否手动添加
}
```

#### AutoMemory（记忆管理器）

位置：`packages/core/src/memory/types.rs:225-242`

```rust
pub struct AutoMemory {
    pub entries: Vec<MemoryEntry>,
    pub config: MemoryConfig,
    pub max_entries: usize,                  // 默认 100
    pub min_importance: f64,                 // 默认 30.0
    pub enabled: bool,
    search_index: Option<SearchIndex>,       // TF-IDF 索引（不持久化）
}
```

### 记忆分类

位置：`packages/core/src/memory/types.rs:39-62`

| 分类 | 图标 | 默认重要性 | 说明 |
|------|------|------------|------|
| `Decision` | 🎯 | 85.0 | 项目决策（如"采用 PostgreSQL"） |
| `Preference` | 👤 | 70.0 | 用户偏好（如"偏好详细解释"） |
| `Solution` | 🔧 | 80.0 | 问题解决方案 |
| `Finding` | 💡 | 65.0 | 重要发现 |
| `Technical` | 📚 | 60.0 | 技术栈信息 |
| `Structure` | 🏗️ | 55.0 | 项目结构 |
| `KeyDecision` | ⚡ | 85.0 | 关键决策 |
| `FailedApproach` | ❌ | 70.0 | 失败方案（避免重复） |
| `UserIntentPattern` | 🧠 | 80.0 | 用户意图模式 |
| `TaskPattern` | 📋 | 75.0 | 任务完成模式 |

### Memory 加载流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      启动时记忆加载完整流程                                   │
└─────────────────────────────────────────────────────────────────────────────┘

main.rs:666-755
│
│  1. 创建 MemoryStorage
│     packages/core/src/memory/storage.rs:178-186
│     ────────────────────────────────────────────────────────────────────────
│     MemoryStorage::new(project_path)
│       ├─ base_dir = ~/.matrix
│       ├─ project_root = cwd 或 session.project_path
│       └─ MemoryFileLock::new()
│
│  2. 加载合并记忆 ★
│     packages/core/src/memory/storage.rs:269-284
│     ────────────────────────────────────────────────────────────────────────
│     load_combined()
│       ├─ load_global() → ~/.matrix/memory.json
│       │   不存在 → AutoMemory::new() (空)
│       │   存在 → 解析 JSON
│       │
│       ├─ load_project() → {project}/.matrix/memory.json
│       │   不存在 → None
│       │   存在 → 解析 JSON
│       │
│       └─ 合并: global.entries + project.entries (标记 "project" tag)
│           prune() → 确保不超过 max_entries
│
│  3. 发送 MemoryLoaded 事件
│     main.rs:672-682
│     ────────────────────────────────────────────────────────────────────────
│     TUI 接收并显示记忆条数
│
│  4. 生成初始 memory summary
│     packages/core/src/memory/types.rs:931-957
│     ────────────────────────────────────────────────────────────────────────
│     generate_prompt_summary(20)
│       ├─ top_n(20) → 按重要性排序
│       ├─ 按类别分组
│       └─ 格式化: "【自动记忆摘要】\n🎯 决策: ...\n👤 偏好: ..."
│
│  5. 构建 System Prompt ★
│     packages/core/src/prompt.rs:442-482
│     ────────────────────────────────────────────────────────────────────────
│     build_system_prompt()
│       ├─ static prompt
│       ├─ tools prompt
│       ├─ project overview
│       ├─ memory summary ← 记忆注入位置
│       └─ skills
│
│  6. 首次项目分析（条件触发）
│     packages/core/src/memory/project.rs:231-246
│     ────────────────────────────────────────────────────────────────────────
│     if !{project}/.matrix/memory.json.exists():
│       ├─ detect_project_type() → Cargo.toml/package.json/go.mod 等
│       ├─ generate_memories() → Technical + Structure 记忆
│       └─ add_entry() → 保存到项目记忆文件
```

### 记忆提取

#### 智能提取流程

位置：`packages/core/src/memory/extractor.rs:257-302`

```
detect_memories_smart(text, session_id, extractor)
│
│  优先级：
│  ① AI 提取 (text > 200 chars + fast_provider 可用)
│     ────────────────────────────────────────────────────────────────────────
│     AiMemoryExtractor::extract()
│       ├─ 发送到 fast_model (默认 haiku)
│       ├─ Prompt: 记忆提取 + JSON 输出格式
│       └─ parse_memory_response()
│           返回: MemoryEntry[] { category, content, importance, keywords, tags }
│
│  ② 规则提取 (fallback)
│     ────────────────────────────────────────────────────────────────────────
│     detect_memories_fallback()
│       ├─ KeywordsConfig 模式匹配
│       └─ 检测关键词: "决定", "偏好", "解决", "发现" 等
```

#### AI 提取 Prompt

位置：`packages/core/src/memory/extractor.rs:38-70`

```text
你是一个记忆提取助手。从对话中提取值得长期记忆的关键信息。

记忆类型：
- decision: 项目或技术选型的决定
- preference: 用户习惯或偏好
- solution: 解决问题的具体方法
- finding: 重要发现或信息
- technical: 技术栈或框架信息
- structure: 项目结构信息

输出格式（严格 JSON）：
{
  "memories": [
    {
      "category": "decision",
      "content": "采用 PostgreSQL 作为主数据库",
      "importance": 85,
      "keywords": ["PostgreSQL", "数据库", "database"],
      "tags": ["backend", "storage"]
    }
  ]
}
```

### Memory 存储机制

**存储路径**：
- 全局记忆：`~/.matrix/memory.json`
- 项目记忆：`{project}/.matrix/memory.json`

**添加记忆流程**（`add()`）：

位置：`packages/core/src/memory/types.rs:396-418`

```rust
pub fn add(&mut self, entry: MemoryEntry) {
    // 1. 重复检查（Jaccard similarity >= 0.7）
    if self.has_similar(&entry.content) {
        return;  // 跳过重复
    }

    // 2. 冲突处理（如 "使用 X" vs "使用 Y"）
    if let Some(conflict_idx) = self.find_conflict(&entry.content, entry.category) {
        self.entries.remove(conflict_idx);  // 新内容覆盖旧内容
    }

    // 3. 添加并修剪
    self.entries.push(entry);
    self.prune();  // 超过 max_entries 时移除低重要性
}
```

**冲突检测逻辑**：

位置：`packages/core/src/memory/types.rs:447-497`

- 计算词重叠率（topic_overlap）
- 检查矛盾信号（"改用", "换成", "不再"）
- Jaccard 相似度 < 0.7 时判定为冲突

### 检索机制

#### TF-IDF 搜索

位置：`packages/core/src/memory/retrieval.rs:286-473`

特性：
- 中文分词支持（CJK 字符 + 双字窗口）
- IDF 缓存优化
- 多关键词搜索

#### 上下文摘要生成

位置：`packages/core/src/memory/types.rs:959-1044`

```rust
pub fn generate_contextual_summary(&self, context: &str, max_entries: usize) -> String {
    // 1. 提取上下文关键词
    let keywords = extract_context_keywords(context);

    // 2. TF-IDF 搜索匹配
    let tfidf_results = tfidf.search_multi(&keywords);

    // 3. 计算综合得分
    //    combined = tfidf * 0.4 + relevance * 0.6
    //    final = combined * 0.6 + (importance/100) * 0.4

    // 4. 手动添加的记忆优先

    // 5. 生成分类摘要文本
}
```

---

## 完整代码路径

### 入口到 Session 加载

| 步骤 | 文件位置 | 函数 |
|------|----------|------|
| ① 程序入口 | `packages/cli/src/main.rs:250` | `main()` |
| ② TUI 入口 | `packages/cli/src/main.rs:497` | `run_terminal_mode(cli)` |
| ③ SessionManager 创建 | `packages/cli/src/main.rs:540` | `SessionManager::new()` |
| ④ 初始化 | `packages/core/src/session.rs:491-506` | `SessionManager::new()` |
| ⑤ 加载索引 | `packages/core/src/session.rs:543-556` | `load_index()` |
| ⑥ 新建/恢复 | `packages/core/src/session.rs:580-612` | `start_new()` / `resume()` |

### 入口到 Memory 加载

| 步骤 | 文件位置 | 函数 |
|------|----------|------|
| ① Storage 创建 | `packages/cli/src/main.rs:668` | `MemoryStorage::new()` |
| ② 初始化 | `packages/core/src/memory/storage.rs:178-186` | `MemoryStorage::new()` |
| ③ 合并加载 | `packages/cli/src/main.rs:669-670` | `load_combined()` |
| ④ 加载实现 | `packages/core/src/memory/storage.rs:269-284` | `load_combined()` |
| ⑤ 生成摘要 | `packages/cli/src/main.rs:684-687` | `generate_prompt_summary()` |
| ⑥ 摘要实现 | `packages/core/src/memory/types.rs:931-957` | `generate_prompt_summary()` |
| ⑦ 构建 Prompt | `packages/cli/src/main.rs:698-704` | `build_system_prompt()` |
| ⑧ Prompt 实现 | `packages/core/src/prompt.rs:442-482` | `build_system_prompt()` |
| ⑨ 注入记忆 | `packages/core/src/prompt.rs:467-471` | `[ACCUMULATED MEMORY]` |

### Memory 动态更新

| 步骤 | 文件位置 | 函数 |
|------|----------|------|
| ① 动态检索 | `packages/cli/src/main.rs:1486-1518` | 每轮对话前 |
| ② AI 提取 | `packages/cli/src/main.rs:1578-1604` | `detect_memories_smart()` |
| ③ 提取实现 | `packages/core/src/memory/extractor.rs:257-302` | `detect_memories_smart()` |
| ④ AI 提取器 | `packages/core/src/memory/extractor.rs:74-117` | `AiMemoryExtractor::extract()` |
| ⑤ 添加记忆 | `packages/core/src/memory/types.rs:396-418` | `AutoMemory::add()` |
| ⑥ 保存记忆 | `packages/core/src/memory/storage.rs:341-356` | `add_entry()` |

---

## 数据流向图

```
                    ┌─────────────────────────────────────────────────────┐
                    │                    用户输入                          │
                    └───────────────────────┬─────────────────────────────┘
                                            │
            ┌───────────────────────────────┼───────────────────────────────┐
            │                               │                               │
            ▼                               ▼                               ▼
    ┌───────────────┐               ┌───────────────┐               ┌───────────────┐
    │ Session       │               │ Memory        │               │ Agent         │
    │               │               │               │               │               │
    │ 无操作        │               │ 动态检索      │               │ messages.push │
    │               │               │ ───────────── │               │               │
    │               │               │ 关键词提取    │               │               │
    │               │               │ TF-IDF 搜索   │               │               │
    │               │               │ 上下文摘要    │               │               │
    │               │               │ ↓             │               │               │
    │               │               │ 更新 sys_prompt│               │               │
    └───────────────┘               └───────────────┘               └───────────────┘
            │                               │                               │
            │                               │                               │
            │                               │                               ▼
            │                               │                       ┌───────────────┐
            │                               │                       │ API 调用      │
            │                               │                       │ + 工具执行    │
            │                               │                       └───────────────┘
            │                               │                               │
            │                               │                               │
            ▼                               ▼                               ▼
    ┌───────────────┐               ┌───────────────┐               ┌───────────────┐
    │ 自动保存      │               │ AI 提取       │               │ get_messages()│
    │ ───────────── │               │ ───────────── │               │               │
    │ set_messages()│               │detect_memories│               │               │
    │ update_stats()│               │ _smart()      │               │               │
    │ save_current()│               │ ───────────── │               │               │
    │               │               │ • AI 提取     │               │               │
    │               │               │ • 规则 fallback│               │               │
    │               │               │               │               │               │
    │               │               │ add_entry()   │               │               │
    │               │               │ ───────────── │               │               │
    │               │               │ 重复检查      │               │               │
    │               │               │ 冲突处理      │               │               │
    │               │               │ 修剪          │               │               │
    │               │               │               │               │               │
    │               │               │ save_global() │               │               │
    │               │               │ /save_project()│               │               │
    └───────────────┘               └───────────────┘               └───────────────┘
            │                               │                               │
            │                               │                               │
            ▼                               ▼                               ▼
    ┌───────────────────────────────────────────────────────────────────────────┐
    │                              文件系统                                       │
    │                                                                             │
    │  ~/.matrix/sessions/{id}.json     ~/.matrix/memory.json (全局)            │
    │                                    {project}/.matrix/memory.json (项目)    │
    └───────────────────────────────────────────────────────────────────────────┘
```

---

## 关键时机总结

| 事件 | Session | Memory |
|------|---------|--------|
| **启动时** | `start_new()` 或 `resume()` | `load_combined()` + 首次项目分析 |
| **每轮对话前** | 无 | 动态检索：关键词提取 → 上下文摘要 → 更新 system_prompt |
| **每轮对话后** | `save_current()` (自动) | AI 提取 assistant 消息 → `add_entry()` |
| **每 5 轮** | 无 | 行为推断：分析工具使用模式 |
| **每 10 轮** | 无 | 定期清理：time_decay + smart_merge + prune |
| **用户反馈** | 无 | `detect_feedback_patterns()` → 更新/标记 |
| **手动命令** | `/new`, `/save`, `/sessions` | `/memory add/forget/analyze/merge` |

---

## 附录：文件路径索引

| 类型 | 路径 | 说明 |
|------|------|------|
| 全局记忆 | `~/.matrix/memory.json` | 跨项目共享记忆 |
| 项目记忆 | `{project}/.matrix/memory.json` | 项目特定记忆 |
| Session 索引 | `~/.matrix/sessions/index.json` | 所有 session 元数据 |
| Session 文件 | `~/.matrix/sessions/{id}.json` | 单个 session 数据 |
| 配置文件 | `~/.matrix/config.json` | MatrixCode 配置 |
| 项目概述 | `{project}/MATRIX.md` | 项目文档 |
| Memory 锁 | `~/.matrix/memory.lock` | 写入锁文件 |
| Session 锁 | `~/.matrix/sessions.lock` | Session 写入锁 |

---

## 已修复的问题

### v0.4.13 修复记录

#### 1. 锁超时返回值问题 ✅

**问题**：`acquire()` 返回 `Ok(false)` 表示超时，但调用方未检查返回值，可能导致写入操作在没有锁的情况下执行。

**修复**：
- `MemoryFileLock::acquire()` 从 `Result<bool>` 改为 `Result<()>`
- `SessionFileLock::acquire()` 同样修改
- 超时时返回 `bail!()` 错误，强制调用方处理

**影响文件**：
- `packages/core/src/memory/storage.rs`
- `packages/core/src/session.rs`

#### 2. add_memory() 双重重复检查 ✅

**问题**：`add_memory()` 在调用 `add()` 前进行了重复检查，`add()` 内部又检查一次，浪费性能。

**修复**：`add_memory()` 直接委托给 `add()`，移除前置检查。

**影响文件**：`packages/core/src/memory/types.rs`

#### 3. AI 提取缺少 project_path ✅

**问题**：AI 提取的记忆没有设置 `project_path`，导致项目记忆归属混乱。

**修复**：
- `MemoryExtractor::extract()` 添加 `project_path` 参数
- `detect_memories_smart()` 添加 `project_path` 参数
- `parse_memory_response()` 传入 `project_path`

**影响文件**：
- `packages/core/src/memory/extractor.rs`
- `packages/cli/src/main.rs`

#### 4. Session 清理机制 ✅

**问题**：Session 文件无限累积，缺少清理机制。

**修复**：
- 新增 `cleanup_old_sessions(max_age_days)` - 删除超过 N 天的 session
- 新增 `prune_sessions(max_sessions)` - 保留最近 N 个 session
- 新增命令 `/sessions cleanup` 和 `/sessions stats`

**影响文件**：
- `packages/core/src/session.rs`
- `packages/cli/src/main.rs`

#### 5. 记忆划分逻辑优化 ✅

**问题**：只要有 `agent_project_path` 就认为是项目记忆，逻辑过于简单，用户偏好等全局记忆被错误存储到项目。

**修复**：
- Preference、UserIntentPattern、TaskPattern → 强制全局
- Decision、Technical、Structure 等 → 根据 project_path 判断

**影响文件**：`packages/cli/src/main.rs`

#### 6. project.rs MemoryEntry 缺少 project_path ✅

**问题**：项目结构分析生成的记忆没有 `project_path`，虽然保存到项目文件，但字段缺失。

**修复**：`generate_memories()` 使用 `self.project_root` 设置 `project_path`。

**影响文件**：`packages/core/src/memory/project.rs`

#### 7. MemoryEntry::manual() 缺少 project_path ✅

**问题**：手动添加记忆没有 `project_path` 参数。

**修复**：
- 新增 `MemoryEntry::manual(category, content, project_path)`
- 新增 `MemoryEntry::manual_global(category, content)` 快捷方法

**影响文件**：
- `packages/core/src/memory/types.rs`
- `packages/core/src/memory/learning.rs`
- `packages/cli/src/main.rs`

#### 8. AI 提取策略优化 ✅

**问题**：每轮对话都调用 AI 提取关键词，短消息浪费 API 调用；AI 失败时使用规则匹配可能不准确。

**修复**：
- 只有 text > 200 字符才触发 AI 提取
- AI 失败时跳过规则匹配（不再 fallback）
- 短文本直接跳过记忆检测

**影响文件**：`packages/core/src/memory/extractor.rs`

---

## 版本信息

- 文档生成时间：2026-05-24
- 最后更新时间：2026-05-24 (修复记录)
- 适用版本：MatrixCode v0.4.x
- 相关代码仓库：`packages/core/src/session.rs`, `packages/core/src/memory/`