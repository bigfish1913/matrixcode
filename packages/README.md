# MatrixCode

> AI代码助手，支持多模型和工具调用

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## 📖 项目简介

MatrixCode是一个用Rust编写的AI代码助手，提供：

- ✅ **多模型支持**: Anthropic Claude、OpenAI GPT
- ✅ **工具调用**: bash命令、文件操作、用户询问
- ✅ **流式响应**: 实时显示AI思考和响应过程
- ✅ **会话管理**: 持久化、恢复、历史记录
- ✅ **上下文压缩**: 自动压缩长对话，节省token
- ✅ **记忆系统**: 项目概览、关键词记忆

---

## 🚀 快速开始

### 安装

```bash
# 从源码编译
git clone <repository-url>
cd matrixcode
cargo build --release

# 安装到系统
cargo install --path packages/cli
```

### 配置

创建配置文件 `~/.matrix/config.json`：

```json
{
  "provider": "anthropic",
  "api_key": "your-api-key-here",
  "model": "claude-sonnet-4-20250514",
  "base_url": "https://api.anthropic.com",
  "think": true,
  "max_tokens": 16384,
  "approve_mode": "ask"
}
```

或使用环境变量：

```bash
export API_KEY="your-api-key"
export MODEL="claude-sonnet-4-20250514"
export PROVIDER="anthropic"
```

### 运行

```bash
# 启动交互式会话
matrixcode

# 恢复上次会话
matrixcode --continue

# 选择会话恢复
matrixcode --resume

# 查看会话列表
matrixcode --list-sessions
```

---

## 🎯 核心功能

### 1. 多模型支持

支持多个AI提供商：

- **Anthropic**: Claude系列（推荐）
  - claude-sonnet-4-20250514（快速、平衡）
  - claude-opus-4（复杂推理）
  
- **OpenAI**: GPT系列
  - gpt-4o（推荐）
  - gpt-4-turbo

自动推断提供商：根据model名称自动选择对应API。

### 2. 工具调用

AI可以使用以下工具：

| 工具 | 功能 | 安全级别 |
|------|------|---------|
| `bash` | 执行shell命令 | 🟡 需审批 |
| `read` | 读取文件内容 | 🟢 安全 |
| `write` | 写入文件内容 | 🟡 需审批 |
| `edit` | 编辑文件（字符串替换） | 🟡 需审批 |
| `multi_edit` | 批量编辑多个位置 | 🟡 需审批 |
| `grep` | 搜索文件内容 | 🟢 安全 |
| `glob` | 查找文件 | 🟢 安全 |
| `ls` | 列出目录内容 | 🟢 安全 |
| `ask` | 向用户提问 | 🟢 安全 |

**审批模式** (`approve_mode`):
- `ask`: 每次工具调用询问用户（推荐）
- `auto`: 自动批准安全操作
- `strict`: 严格拒绝��险操作

### 3. 流式响应

实时显示AI的思考和响应：

```
💭 Thinking: 分析代码结构...
   → 需要重构Agent模块
   → 先拆分Config和State

💬 Response: 我建议将Agent拆分为...
```

### 4. 会话管理

**会话持久化**:
- 会话自动保存到 `~/.matrix/sessions/`
- 包含消息历史、token统计、项目路径

**会话恢复**:
```bash
# 列出所有会话
matrixcode --list-sessions

# 交互式选择会话
matrixcode --resume

# 恢复特定会话
matrixcode --resume-id <session-id>

# 继续上次会话
matrixcode --continue-session
```

### 5. 上下文压缩

自动压缩长对话：
- 检测token数量接近上下文限制
- 使用滑动窗口策略压缩
- 保留关键信息和最近对话
- 显示压缩进度和压缩率

### 6. 记忆系统

**项目概览** (`/init`):
```bash
# 生成项目结构概览
/init

# 查看概览状态
/init status

# 重置概览
/init reset
```

**关键词记忆**:
- 自动提取重要信息
- 存储到 `~/.matrix/memory.json`
- 在后续对话中使用

---

## 📁 项目结构

```
matrixcode/
├── packages/
│   ├── core/           # 核心逻辑库
│   │   ├── src/
│   │   │   ├── agent/  # Agent核心逻辑
│   │   │   ├── providers/ # API提供商
│   │   │   ├── tools/  # 工具实现
│   │   │   ├── memory/ # 记忆系统
│   │   │   ├── compress/ # 上下文压缩
│   │   │   ├── config.rs # 配置管理
│   │   │   └── session.rs # 会话管理
│   │   └── Cargo.toml
│   ├── cli/            # 命令行工具
│   │   ├── src/
│   │   │   ├── main.rs # CLI入口
│   │   │   └── display.rs # 显示逻辑
│   │   └── Cargo.toml
│   ├── tui/            # 终端UI（TUI模式）
│   │   ├── src/
│   │   │   ├── app.rs  # TUI应用
│   │   │   ├── ui.rs   # UI渲染
│   │   │   └── event.rs # 事件处理
│   │   └── Cargo.toml
│   └── vscode/         # VSCode扩展（可选）
├── docs/               # 文档
│   ├── CODE_REVIEW_REPORT.md # 代码审查报告
│   ├── IMPROVEMENT_CHECKLIST.md # 改进清单
│   └── ARCHITECTURE.md # 架构设计（TODO）
└── README.md           # 本文件
```

---

## 🔧 配置说明

### 配置文件格式

`~/.matrix/config.json`:

```json
{
  "provider": "anthropic",
  "api_key": "your-api-key",
  "model": "claude-sonnet-4-20250514",
  "base_url": "https://api.anthropic.com",
  "think": true,
  "markdown": true,
  "max_tokens": 16384,
  "approve_mode": "ask",
  "multi_model": false,
  "plan_model": null,
  "compress_model": null,
  "extra_headers": {}
}
```

### 配置字段说明

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| `provider` | string | API提供商: "anthropic" 或 "openai" | 自动推断 |
| `api_key` | string | API密钥 | 无（必须） |
| `model` | string | 模型名称 | claude-sonnet-4 |
| `base_url` | string | API端点URL | https://api.anthropic.com |
| `think` | bool | 启用扩展思考 | true |
| `markdown` | bool | 启用Markdown渲染 | true |
| `max_tokens` | int | 最大输出token数 | 16384 |
| `approve_mode` | string | 工具审批模式 | "ask" |
| `multi_model` | bool | 启用多模型配置 | false |
| `extra_headers` | object | 额外HTTP头 | {} |

### 环境变量

也支持通过环境变量配置：

```bash
# 通用变量（推荐）
export API_KEY="your-key"
export MODEL="claude-sonnet-4"
export PROVIDER="anthropic"
export BASE_URL="https://api.anthropic.com"

# 提供商特定变量
export ANTHROPIC_AUTH_TOKEN="your-key"  # Anthropic专用
export ANTHROPIC_MODEL="claude-sonnet-4"
export OPENAI_API_KEY="your-key"         # OpenAI专用

# 其他配置
export APPROVE_MODE="ask"
export MAX_TOKENS="16384"
export THINK="true"
```

**优先级**: 环境变量 > config.json > 默认值

---

## 💡 使用示例

### 示例1：代码重构

```
用户: 重构Agent结构体，拆分为Config、State、Context

AI: 
💭 Thinking: 分析当前Agent结构...
   → 识别15个字段，职责混乱
   → 设计拆分方案...

💬 我建议将Agent拆分为：
   - AgentConfig: 配置类字段（provider、model、max_tokens）
   - AgentState: 状态类字段（messages、token统计）
   - AgentContext: 上下文（skills、overview、memory）
   
   是否需要我创建重构代码？
   
工具调用:
   ✓ read core/src/agent/types.rs (读取当前代码)
   ✓ write core/src/agent/config.rs (创建新模块)
   ✓ edit core/src/agent/types.rs (重构Agent)
   
   完成重构，共修改4个文件。
```

### 示例2：Bug修复

```
用户: fix(config): config.rs中unwrap可能导致panic

AI:
💭 Thinking: 定位unwrap使用位置...
   → 第541行：serde_json::from_str(json).unwrap()
   → 第560行：类似问题
   
工具调用:
   ✓ grep "unwrap\(\)" core/src/config.rs (查找所有unwrap)
   ✓ edit core/src/config.rs (替换为Result处理)
   
   修复了3处unwrap：
   - 添加错误上下文信息
   - 配置损坏时返回友��错误
   - 测试通过
```

### 示例3：功能开发

```
用户: 实现文件路径验证，防止路径穿越

AI:
💭 Thinking: 设计路径验证逻辑...
   → 检查路径穿越（../）
   → 确保路径在项目目录内
   
工具调用:
   ✓ write core/src/path_validator.rs (创建验证模块)
   ✓ edit core/src/tools/write.rs (集成验证)
   ✓ write tests/path_validator_test.rs (添加测试)
   
   实现：
   - validate_path() 函数
   - 阻止 ../../../etc/passwd
   - 测试覆盖率100%
```

---

## ⚠️ 安全说明

### 安全边界

MatrixCode的bash工具**不是沙箱环境**：

- ✅ 阻止最明显的灾难性命令（`rm -rf /`、`mkfs`等）
- ❌ 无法防止所有恶意操作
- ⚠️ 执行的命令具有用户级别的权限

**使用建议**:
1. 使用 `approve_mode="ask"` 审查每个命令
2. 不要在root或生产环境运行
3. 定期检查会话日志
4. 对敏感文件设置权限保护

详细安全说明请参考 `docs/SECURITY.md`（TODO）。

---

## 🗺️ 架构概览

### 核心流程

```
用户输入 → CLI解析 → Agent.run()
  ↓
Provider.chat_stream() → 流式响应
  ↓
工具调用（需审批） → 工具执行
  ↓
结果返回 → Agent处理 → 事件发送
  ↓
TUI/CLI显示 → 用户查看
```

### 模块职责

- **core**: 核心逻辑库（无UI依赖）
  - `agent`: Agent核心逻辑、消息处理
  - `providers`: API提供商接口和实现
  - `tools`: 工具定义和执行
  - `memory`: 记忆系统
  - `compress`: 上下文压缩
  - `session`: 会话管理
  
- **cli**: 命令行工具
  - 参数解析（clap）
  - TUI启动和恢复
  - 会话列表显示
  
- **tui**: 终端UI
  - Ratatui框架
  - 事件处理和渲染
  - 用户输入处理

详细架构请参考 `docs/ARCHITECTURE.md`（TODO）。

---

## 📚 文档

- [代码审查报告](docs/CODE_REVIEW_REPORT.md) - 详细的代码review发现
- [改进清单](docs/IMPROVEMENT_CHECKLIST.md) - 22个待改进项，按优先级排序
- [架构设计](docs/ARCHITECTURE.md) - TODO
- [安全说明](docs/SECURITY.md) - TODO
- [API文档](docs/API.md) - TODO

---

## 🧪 测试

```bash
# 运行单元测试
cargo test

# 运行特定测试
cargo test test_agent_message_flow

# 运行集成测试
cargo test --test integration_test

# 测试覆盖率（需要cargo-tarpaulin）
cargo tarpaulin --out Html
```

---

## 🛠️ 开发指南

### 本地开发

```bash
# 克隆项目
git clone <repository-url>
cd matrixcode

# 安装依赖
cargo build

# 运行测试
cargo test

# 运行CLI
cargo run --package matrixcode-cli

# 运行TUI模式
cargo run --package matrixcode-cli -- --mode tui
```

### 代码规范

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 函数不超过50行，嵌套不超过3层
- 结构体字段不超过10个
- 生产代码禁止使用 `.unwrap()`
- 所有外部调用必须有错误处理

### 提交代码

```bash
# 创建feature分支
git checkout -b feature/new-feature

# 提交代码
git add .
git commit -m "feat: 添加新功能描述"

# 运行检查
cargo fmt --check
cargo clippy
cargo test

# 合并到main
git checkout main
git merge feature/new-feature
```

---

## 📊 项目状态

### 当前版本: v0.1.0

### 已完成功能
- ✅ 多模型支持（Anthropic、OpenAI）
- ✅ 工具调用（9个工具）
- ✅ 流式响应和thinking显示
- ✅ 会话管理和恢复
- ✅ 上下文压缩
- ✅ 记忆系统
- ✅ 基本配置系统

### 待改进项
基于代码审查，发现22个待改进项：
- 🔴 高优先级: 8个（安全与稳定性）
- 🟡 中优先级: 7个（架构与质量）
- 🟢 低优先级: 7个（文档与测试）

详见 [改进清单](docs/IMPROVEMENT_CHECKLIST.md)。

---

## 🤝 贡献指南

欢迎贡献代码、报告问题或提出建议！

### 贡献方式
1. Fork项目
2. 创建feature分支
3. 提交改动
4. 确保测试通过
5. 提交Pull Request

### 报告问题
- 使用GitHub Issues
- 描述问题现象、复现步骤、期望结果
- 附上相关日志和配置

---

## 📄 许可证

MIT License

---

## 📞 联系方式

- 项目维护者: [维护者信息]
- GitHub: [项目链接]
- 文档: [文档链接]

---

## 🙏 致谢

感谢以下项目和库：
- [Anthropic Claude API](https://www.anthropic.com/)
- [OpenAI API](https://openai.com/)
- [Ratatui](https://github.com/ratatui-org/ratatui) - TUI框架
- [Tokio](https://tokio.rs/) - 异步运行时
- [Anyhow](https://github.com/dtolnay/anyhow) - 错误处理

---

**MatrixCode - 让AI成为你的代码伙伴** 🤖✨