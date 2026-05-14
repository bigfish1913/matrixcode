# MatrixCode

一个基于 Rust 开发的智能代码代理 CLI 工具，支持多模型配置、自动上下文压缩、任务规划和会话管理。

## 功能特性

- 🤖 **多模型配置**: 支持 main/plan/compress/fast 四种模型角色
- 🗜️ **智能压缩**: 自动检测并压缩超出阈值的上下文
- 📋 **任务规划**: 使用规划模型分解复杂任务
- 💾 **会话管理**: 支持多会话保存、恢复和命名
- 📁 **文件操作**: 读写、编辑、搜索文件
- 🔧 **工具系统**: 可扩展的工具和技能支持
- 🌐 **Web 搜索**: 内置网页搜索能力
- 💻 **跨平台**: Linux、macOS、Windows

## 安装

### 通过 npm 安装（推荐）

```bash
npm install -g matrixcode
```

安装脚本会自动检测并尝试以下方式：
1. 如果已安装 Rust，使用 `cargo install matrixcode`
2. 从 GitHub Releases 下载预编译二进制

### 通过 Cargo 安装

如果你已安装 Rust 工具链：

```bash
cargo install matrixcode
```

### 从 Release 下载

从 [GitHub Releases](https://github.com/bigfish1913/matrixcode/releases) 下载对应平台的预编译二进制文件。

支持的平台：
- macOS (x64, arm64/Apple Silicon)
- Linux (x64, arm64)
- Windows (x64)

### 从源码构建

```bash
git clone https://github.com/bigfish1913/matrixcode.git
cd matrixcode
cargo build --release
```

构建完成后，二进制文件位于 `target/release/matrixcode`。

## 快速开始

### 基本使用

```bash
# 交互模式（进入 REPL）
matrixcode

# 单次问答
matrixcode "帮我分析这个项目的结构"

# 继续上次会话
matrixcode -c

# 恢复指定会话
matrixcode --resume session-abc

# 生成项目概览
matrixcode --init

# 指定模型
matrixcode --provider anthropic --model claude-sonnet-4-20250514
```

### 多模型模式

```bash
# 启用多模型（所有角色默认使用主模型）
matrixcode --multi-model

# 指定不同的模型
matrixcode --multi-model \
  --plan-model claude-sonnet-4-20250514 \
  --compress-model claude-3-5-haiku-20241022 \
  --fast-model claude-3-5-haiku-20241022

# 只指定压缩模型（其他使用主模型）
matrixcode --compress-model claude-3-5-haiku-20241022
```

## REPL 命令

在交互模式中，支持以下命令：

| 命令 | 说明 |
|------|------|
| `/help` | 显示帮助信息 |
| `/status` | 显示会话状态（消息数、token 使用） |
| `/history` | 显示对话历史摘要 |
| `/sessions` | 列出所有保存的会话 |
| `/resume` | 交互式选择会话恢复 |
| `/resume <id>` | 恢复指定会话 |
| `/rename <name>` | 为当前会话命名 |
| `/plan` | 显示或生成任务计划 |
| `/plan <task>` | 为指定任务生成计划 |
| `/models` | 显示当前模型配置 |
| `/compress` | 手动压缩上下文 |
| `/compress <bias>` | 指定偏重压缩 |
| `/clear` | 清空上下文，开始新会话 |
| `/init` | 生成/更新项目概览 |
| `/overview` | 显示项目概览状态 |
| `/exit` | 退出 REPL |

### 压缩偏重选项

```
/compress              # 默认 balanced 偏重
/compress balanced     # 平衡保留（工具、用户问题）
/compress important    # 保留工具、思考、决策关键词
/compress tools        # 侧重保留工具操作
/compress aggressive   # 激进压缩（删除更多）
/compress preserve:tools,thinking keywords:决定,重要  # 自定义
```

## 环境变量配置

复制 `.env.example` 为 `.env`：

```bash
cp .env.example .env
```

### 基础配置

```env
# Provider 设置
PROVIDER=anthropic
API_KEY=sk-ant-your-key-here
MODEL_NAME=claude-sonnet-4-20250514
BASE_URL=                             # 可选，自定义 API 端点
```

### 多模型配置

```env
# 启用多模型模式
MULTI_MODEL=true

# 规划模型（默认使用 MODEL_NAME）
PLAN_MODEL=claude-sonnet-4-20250514

# 压缩模型（推荐使用小模型）
COMPRESS_MODEL=claude-3-5-haiku-20241022

# 快速模型
FAST_MODEL=claude-3-5-haiku-20241022
```

### 压缩配置

```env
# 压缩阈值（0.0-1.0）
COMPRESSION_THRESHOLD=0.75

# 最少保留消息数
MIN_PRESERVE_MESSAGES=6

# 手动指定上下文窗口大小
CONTEXT_SIZE=

# 禁用自动压缩
NO_COMPRESSION=false
```

### 输出配置

```env
# 启用扩展思考
THINK=true

# 最大输出 tokens
MAX_TOKENS=16384

# Markdown 渲染
MARKDOWN=true
```

### 其他配置

```env
# Prompt profile: default, safe, fast, review
PROMPT_PROFILE=default

# 禁用网页搜索
NO_WEB_SEARCH=false

# 每轮最大搜索次数
WEB_SEARCH_MAX_USES=5

# 禁用提示词缓存
NO_CACHING=false
```

## CLI 参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-p, --provider` | 模型提供商 | `anthropic` |
| `-m, --model` | 主模型名称 | `claude-sonnet-4-20250514` |
| `--think` | 启用扩展思考 | `true` |
| `--max-tokens` | 最大输出 tokens | `16384` |
| `--markdown` | Markdown 渲染 | `true` |
| `-c, --continue` | 继续上次会话 | - |
| `--resume <id>` | 恢复指定会话 | - |
| `--list-sessions` | 列出所有会话 | - |
| `--multi-model` | 启用多模型 | `false` |
| `--plan-model` | 规划模型 | 使用主模型 |
| `--compress-model` | 压缩模型 | 使用主模型 |
| `--fast-model` | 快速模型 | 使用主模型 |
| `--compression-threshold` | 压缩阈值 | `0.75` |
| `--no-compression` | 禁用压缩 | `false` |
| `--profile` | Prompt profile | `default` |
| `--no-web-search` | 禁用网页搜索 | `false` |
| `--init` | 生成项目概览 | - |
| `--no-overview` | 禁用项目概览 | `false` |

## 模型角色说明

| 角色 | 用途 | 推荐模型 | 特点 |
|------|------|---------|------|
| **Main** | 任务执行、代码生成 | claude-sonnet-4 | 大上下文、强推理 |
| **Plan** | 任务规划、步骤分解 | claude-sonnet-4 | 理解能力强 |
| **Compress** | 上下文压缩、摘要生成 | claude-3-5-haiku | 小模型、成本低 |
| **Fast** | 快速分类、简单判断 | claude-3-5-haiku | 响应快、成本低 |

## 示例配置

### 单模型模式（默认）

```env
PROVIDER=anthropic
MODEL_NAME=claude-sonnet-4-20250514
```

所有任务都使用同一个模型。

### 多模型 - 全部使用主模型

```env
PROVIDER=anthropic
MODEL_NAME=claude-sonnet-4-20250514
MULTI_MODEL=true
```

启用多模型功能，但所有角色都使用 `MODEL_NAME`，适合简化管理。

### 多模型 - 分工协作

```env
PROVIDER=anthropic
MODEL_NAME=claude-sonnet-4-20250514
MULTI_MODEL=true
PLAN_MODEL=claude-sonnet-4-20250514
COMPRESS_MODEL=claude-3-5-haiku-20241022
FAST_MODEL=claude-3-5-haiku-20241022
```

- 主任务和规划使用大模型
- 压缩和快速操作使用小模型，节省成本

### OpenAI 配置

```env
PROVIDER=openai
MODEL_NAME=gpt-4o
MULTI_MODEL=true
COMPRESS_MODEL=gpt-4o-mini
```

### 国内代理配置

```env
PROVIDER=anthropic
API_KEY=sk-your-key
BASE_URL=https://your-proxy.com
MODEL_NAME=claude-sonnet-4-20250514
MULTI_MODEL=true
COMPRESS_MODEL=claude-3-5-haiku-20241022
```

## 开发

```bash
# 运行测试
cargo test

# 代码检查
cargo clippy

# 格式化
cargo fmt

# 构建 release
cargo build --release
```

## 发布

### 发布到 crates.io

```bash
# 登录 crates.io（首次）
cargo login

# 发布
cargo publish
```

发布后，用户可以通过 `cargo install matrixcode` 安装。

### 发布到 npm

```bash
cd npm
npm login
npm publish
```

发布后，用户可以通过 `npm install -g matrixcode` 安装。

### 发布 GitHub Release

1. 更新版本号（Cargo.toml 和 npm/package.json）
2. 创建 Git tag：
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. GitHub Actions 会自动构建并发布预编译二进制

## 项目结构

```
matrixcode/
├── src/
│   ├── agent.rs       # Agent 核心实现
│   ├── models.rs      # 多模型配置和任务规划
│   ├── compress.rs    # 上下文压缩逻辑
│   ├── session.rs     # 会话管理
│   ├── providers/     # LLM Provider 实现
│   ├── tools/         # 工具系统
│   ├── skills.rs      # 技能系统
│   ├── prompt.rs      # 系统提示词
│   └── ...
├── skills/            # 自定义技能目录
├── .env.example       # 配置模板
└── Cargo.toml
```

## 许可证

MIT License