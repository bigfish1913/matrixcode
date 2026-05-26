# MatrixCode vs Claude Code：有记忆的开源 AI 编程助手

## 引言

Claude Code 是 Anthropic 官方推出的 AI 编程助手 CLI 工具，功能强大但闭源且仅支持 Anthropic API。如果你想要一个**开源、支持多模型、能跨会话记忆**的替代品，MatrixCode 可能是你的最佳选择。

---

## 功能对比

| 功能 | Claude Code | MatrixCode |
|------|-------------|------------|
| **跨会话记忆** | ⚠️ 有但有限 | ✅ **完善的记忆系统**（分类、评分、衰减、冲突检测） |
| **成本优化** | ❌ 单一模型 | ✅ 多模型协作，压缩用小模型可节省 50-70% |
| **开源** | ❌ 闭源 | ✅ **MIT 开源**，可自由修改和部署 |
| **多 Provider** | ❌ 仅 Anthropic | ✅ OpenAI + Anthropic + 国内代理 |
| **私有化部署** | ❌ 云端依赖 | ✅ **本地存储**，数据完全自主 |
| **跨平台** | ✅ | ✅ Linux/macOS/Windows |
| **上下文压缩** | ✅ | ✅ + 多种压缩策略 |
| **任务规划** | ⚠️ 基础 | ✅ 专用规划模型 |
| **中文支持** | ⚠️ 基础 | ✅ 中文提示词、记忆分类 |

---

## MatrixCode 的三大独特优势

### 1. 🧠 跨会话记忆系统

这是 MatrixCode 最大的差异化功能。

**场景演示**：

```bash
# 第一次对话（周一）
> 我决定使用 PostgreSQL 作为这个项目的数据库
[saved 1 new memories]

> 前端用 React，状态管理用 Zustand
[saved 2 new memories]

# 关闭终端...

# 第二次对话（周三）
> 继续设计数据库 schema
[loaded 15 accumulated memories]
# AI 自动记得：
# - 你选择了 PostgreSQL
# - 前端用 React + Zustand
# 不需要重复说明！

# 改变决定
> 改用 MySQL，PostgreSQL 对这个小项目太重了
[saved 1 new memories]
# AI 自动更新记忆，后续不再提 PostgreSQL
```

**记忆类型**：
- 🎯 **决策**：技术选型决定（重要性 90）
- 👤 **偏好**：用户习惯（重要性 70）
- 💡 **发现**：重要信息（重要性 60）
- 🔧 **解决方案**：解决方法（重要性 85）

**智能特性**：
- **冲突检测**：`"改用 X"` 自动覆盖 `"使用 Y"`
- **时间衰减**：旧记忆重要性降低
- **引用增加**：常用记忆重要性提升
- **上下文检索**：根据对话内容选择相关记忆

### 2. 💰 多模型协作 = 成本节省

Claude Code 始终使用单一的大模型（如 Claude Sonnet），即使做简单任务也消耗大量 token。

MatrixCode 可以配置**多个模型分工**：

```env
# 主任务用大模型
MODEL_NAME=claude-sonnet-4-20250514

# 压缩、快速判断用小模型
COMPRESS_MODEL=claude-3-5-haiku-20241022
FAST_MODEL=claude-3-5-haiku-20241022
```

**节省原理**：
- 上下文压缩（高频操作）→ Haiku（成本 ≈ Sonnet 的 1/10）
- 快速分类、简单判断 → Haiku
- 主任务、复杂推理 → Sonnet

**实测节省**：对于长时间编程任务，可节省 **50-70% token 成本**。

### 3. 🔐 开源 + 数据自主

**开源优势**：
- 可自由修改和定制
- 可添加新工具和技能
- 可集成到内部系统

**数据自主**：
- 记忆存储在本地 `~/.matrix/memory.json`
- 会话存储在本地 `.matrix/sessions/`
- 不依赖云端，可完全离线使用（配合本地模型）

**私有化部署**：
```env
# 使用国内代理
PROVIDER=anthropic
BASE_URL=https://your-proxy.com

# 或使用 OpenAI
PROVIDER=openai
MODEL_NAME=gpt-4o
```

---

## 使用体验对比

### Claude Code 体验

```bash
# 安装
npm install -g @anthropic-ai/claude-code

# 使用
claude "帮我写一个 React 组件"
# 只能用 Anthropic API
```

### MatrixCode 体验

```bash
# 安装（多种方式）
npm install -g @bigfishnpm/matrixcode     # npm
cargo install matrixcode      # Rust
# 或下载预编译二进制

# 使用（支持多 Provider）
matrixcode --provider anthropic "帮我写一个 React 组件"
matrixcode --provider openai "帮我写一个 React 组件"

# 记忆命令
/memory show      # 查看所有记忆
/memory search    # 搜索记忆
/memory add       # 手动添加
```

---

## 适合谁用？

### 推荐 Claude Code

- 只用 Anthropic API
- 不关心成本
- 不需要私有化部署
- 官方支持更重要

### 推荐 MatrixCode

- **想要跨会话记忆**（记住你的项目决策）
- **想要成本优化**（多模型分工）
- **想要开源和私有化**
- **想要支持国内代理/OpenAI**
- **想要定制和扩展**

---

## 快速开始

```bash
# 安装
npm install -g @bigfishnpm/matrixcode

# 配置
cp .env.example .env
# 编辑 .env，填写 API_KEY

# 开始使用
matrixcode

# 第一次对话会自动积累记忆
> 这个项目用 TypeScript，后端是 Node.js
[saved 2 new memories]

# 下次对话自动加载记忆
> 帮我写一个 Express 路由
[loaded 2 accumulated memories]
# AI 自动知道用 TypeScript + Node.js
```

---

## 项目信息

- **GitHub**: https://github.com/bigfish1913/matrixcode
- **许可证**: MIT
- **语言**: Rust（高性能单二进制）
- **平台**: Linux, macOS, Windows

---

## 总结

如果你需要：
- **跨会话记忆** → MatrixCode ✅
- **成本优化** → MatrixCode ✅
- **开源私有化** → MatrixCode ✅
- **多 Provider** → MatrixCode ✅

如果你只需要：
- Anthropic 官方工具 → Claude Code ✅

**MatrixCode = 开源 + 有记忆 + 省成本的 AI 编程助手**

---

*欢迎在 GitHub Star、Issue 和 PR！*