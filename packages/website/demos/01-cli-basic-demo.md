# MatrixCode CLI 使用 Demo

这是一个完整的 CLI 使用演示项目，展示如何使用 MatrixCode 进行实际开发工作。

## 📋 项目目标

创建一个简单的 CLI 工具，实现以下功能：
- 解析命令行参数
- 处理文件内容
- 输出格式化结果

## 🚀 完整工作流程

### 1. 项目初始化

```bash
# 创建项目目录
mkdir calculator-demo
cd calculator-demo

# 初始化 Rust 项目
cargo init
```

### 2. 启动 MatrixCode CLI

```bash
# 启动交互式会话
matrixcode
```

### 3. 实际对话过程

**用户输入：**
```
帮我创建一个简单的计算器 CLI 工具，支持加减乘除四则运算。
需要处理命令行参数，使用 clap 库。
```

**MatrixCode 响应：**
```
💭 Thinking: 分析需求...
   → 需要创建 CLI 应用
   → 使用 clap 解析参数
   → 实现四则运算逻辑

我来帮你创建这个计算器工具。

🔍 code_search: 查找现有项目结构...
📁 ls: 查看文件列表...
📖 read: 读取 Cargo.toml...

✏️ edit: Cargo.toml
添加 clap 依赖:
[dependencies]
clap = { version = "4.0", features = ["derive"] }

✏️ write: src/main.rs
创建 CLI 应用代码:
use clap::Parser;

#[derive(Parser)]
#[command(name = "calculator")]
#[command(about = "Simple calculator CLI")]
struct Cli {
    /// First number
    #[arg(short, long)]
    a: f64,
    
    /// Second number
    #[arg(short, long)]
    b: f64,
    
    /// Operation: add, sub, mul, div
    #[arg(short, long)]
    op: String,
}

fn main() {
    let cli = Cli::parse();
    
    let result = match cli.op.as_str() {
        "add" => cli.a + cli.b,
        "sub" => cli.a - cli.b,
        "mul" => cli.a * cli.b,
        "div" => {
            if cli.b == 0.0 {
                eprintln!("Error: Division by zero");
                return;
            }
            cli.a / cli.b
        },
        _ => {
            eprintln!("Error: Unknown operation '{}'", cli.op);
            return;
        }
    };
    
    println!("Result: {} {} {} = {}", cli.a, cli.op, cli.b, result);
}

✅ 完成！现在可以测试使用。
```

### 4. 编译和测试

**用户输入：**
```
帮我编译并测试这个程序
```

**MatrixCode 响应：**
```
🔧 bash: cargo build --release
   Compiling calculator-demo v0.1.0
   Finished release [optimized] target(s)

✅ 编译成功！

🔧 bash: cargo run -- --a 10 --b 5 --op add
   Result: 10 add 5 = 15

🔧 bash: cargo run -- --a 10 --b 5 --op mul
   Result: 10 mul 5 = 50

🔧 bash: cargo run -- --a 10 --b 0 --op div
   Error: Division by zero

✅ 测试通过！程序正常工作。
```

### 5. 添加更多功能

**用户输入：**
```
现在添加一个 --help 功能，显示使用说明
```

**MatrixCode 响应：**
```
✏️ edit: src/main.rs
clap 已经自动生成 --help，让我测试一下:

🔧 bash: cargo run -- --help
calculator 0.1.0
Simple calculator CLI

Usage: calculator [OPTIONS] --a <A> --b <B> --op <OP>

Options:
  -a, --a <A>      First number
  -b, --b <B>      Second number
  -o, --op <OP>    Operation: add, sub, mul, div
  -h, --help       Print help

✅ --help 功能已经自动可用！clap 库会自动生成帮助文档。
```

## ✨ 关键要点

1. **自动分析**: MatrixCode 自动分析需求，制定实施计划
2. **工具调用**: 使用多个工具（edit, write, bash）完成任务
3. **实时验证**: 编译和测试都在对话中完成
4. **智能建议**: 自动选择合适的库（clap）和最佳实践

## 🎯 实际命令总结

整个流程中使用的 MatrixCode 命令：

| 阶段 | 用户输入 | Agent 工具调用 | 结果 |
|------|---------|---------------|------|
| 初始化 | "创建计算器CLI" | code_search, ls, read | 分析项目 |
| 实现 | (自动) | edit Cargo.toml, write main.rs | 编写代码 |
| 测试 | "编译并测试" | bash cargo build, cargo run | 验证功能 |
| 扩展 | "添加help功能" | bash cargo run --help | 确认已有 |

## 📊 效率对比

| 方式 | 时间 | 步骤 |
|------|------|------|
| 手动开发 | ~30分钟 | 查文档、写代码、调试、测试 |
| MatrixCode | ~5分钟 | 描述需求，自动完成所有步骤 |

**节省时间**: 83% (25分钟)

## 🔗 相关文档

- [MatrixCode 工具文档](../docs.html#tools)
- [更多示例](../examples.html)
- [完整配置说明](../docs.html#config)