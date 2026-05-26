# MatrixCode CLI

智能代码代理 CLI 工具，支持多模型配置、自动上下文压缩、任务规划和会话管理。

## 安装

### 通过 npm 安装（推荐）

```bash
npm install -g @bigfishnpm/matrixcode
```

### 通过 Cargo 安装

```bash
cargo install matrixcode
```

### 从源码构建

```bash
cd packages/cli
cargo build --release
```

## 使用

```bash
# 交互模式
matrixcode

# 单次问答
matrixcode "帮我分析这个项目"

# JSON 输出模式（VSCode 插件集成）
matrixcode --json "你的问题"

# Daemon 模式
matrixcode --daemon

# 继续上次会话
matrixcode -c
```

## 配置

复制 `.env.example` 为 `.env`：

```bash
cp .env.example .env
```

配置 API Key：

```env
PROVIDER=anthropic
API_KEY=sk-ant-your-key-here
MODEL_NAME=claude-sonnet-4-20250514
```

## 开发

```bash
# 运行测试
cargo test

# 构建
cargo build

# 发布
cargo publish
cd npm && npm publish --access public
```

## 文档

完整文档见 [../docs/](../docs/) 目录。

## 许可证

MIT License