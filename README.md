# MatrixCode

一个基于 Rust 开发的智能代码代理 CLI 工具。

## 功能特性

- 🤖 支持多种 LLM 提供商（OpenAI、Anthropic）
- 📁 智能文件系统操作（读写、编辑、搜索）
- 🔧 可扩展的工具系统
- 🎯 技能系统支持
- 💻 跨平台支持（Linux、macOS、Windows）

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/yourusername/matrixcode.git
cd matrixcode

# 构建
cargo build --release

# 二进制文件位于 target/release/matrixcode
```

## 使用

```bash
# 交互模式
./matrixcode

# 单次执行
./matrixcode "your prompt here"

# 生成项目概览
./matrixcode --init

# 指定模型
./matrixcode --provider anthropic --model claude-3-opus-20240229
```

## 配置

复制 `.env.example` 为 `.env` 并填写配置：

```bash
cp .env.example .env
```

主要配置项：

- `PROVIDER`: 模型提供商 (openai / anthropic)
- `API_KEY`: API 密钥
- `MODEL_NAME`: 模型名称
- `BASE_URL`: 可选，自定义 API 端点

## 开发

```bash
# 运行测试
cargo test

# 代码检查
cargo clippy

# 格式化
cargo fmt
```

## 许可证

MIT License