# MatrixCode 快速开发指南

## 快速开始

### 1分钟启动
```bash
cd packages/cli
make build    # 编译
make test     # 测试
make run      # 运行
```

### 常用命令
```bash
make build      # 编译
make test       # 测试
make run        # 运行
make daemon     # 测试daemon
make clean      # 清理
make check      # 代码检查
make fmt        # 格式化
make help       # 显示帮助
```

## 项目结构

```
packages/cli/
├── crates/
│   ├── matrixcode-core/   # Agent核心
│   ├── matrixcode-tui/    # Terminal UI
│   └── matrixcode-cli/    # CLI入口
├── npm/                   # npm发布包
├── docs/                  # 文档
├── Cargo.toml             # Workspace配置
└── Makefile               # 常用命令
```

## 核心模块

| 模块 | 文件 | 功能 |
|------|------|------|
| Agent | agent.rs | 事件驱动Agent |
| Event | event.rs | AgentEvent协议 |
| Provider | providers/ | Anthropic/OpenAI |
| Tools | tools/ | 13个工具 |
| Config | config.rs | 配置加载 |

## 发布流程

```bash
make build && make test && make publish
```

## 文档索引

| 文档 | 说明 |
|------|------|
| PROJECT_FINAL.md | 项目完成总结 |
| PROJECT_CLEANUP.md | 清理指南 |
| FEATURE_COMPLETE.md | 功能完成 |
