#!/bin/bash
# CLI Basic Demo 测试脚本
# 验证 MatrixCode CLI 工作流程

set -e  # 遇到错误立即退出

echo "🧪 开始测试 CLI Basic Demo..."

# 创建测试目录
TEST_DIR="test-calculator-demo"
rm -rf "$TEST_DIR"
mkdir "$TEST_DIR"
cd "$TEST_DIR"

echo "✅ 测试环境已创建"

# 初始化 Rust 项目
cargo init --name calculator-demo
echo "✅ Cargo 项目已初始化"

# 添加 clap 依赖（模拟 MatrixCode 的 edit 操作）
cat > Cargo.toml << 'EOF'
[package]
name = "calculator-demo"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.0", features = ["derive"] }
EOF

echo "✅ Cargo.toml 已更新"

# 创建主程序（模拟 MatrixCode 的 write 操作）
cat > src/main.rs << 'EOF'
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
EOF

echo "✅ src/main.rs 已创建"

# 编译项目（模拟 MatrixCode 的 bash 工具）
echo "📦 编译项目..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ 编译成功"
else
    echo "❌ 编译失败"
    exit 1
fi

# 运行测试（模拟 MatrixCode 的测试流程）
echo "🧪 运行功能测试..."

# 测试加法
RESULT=$(cargo run --release -- --a 10 --b 5 --op add 2>&1)
EXPECTED="Result: 10 add 5 = 15"
if [[ "$RESULT" == "$EXPECTED" ]]; then
    echo "✅ 加法测试通过: $RESULT"
else
    echo "❌ 加法测试失败"
    echo "   期望: $EXPECTED"
    echo "   实际: $RESULT"
    exit 1
fi

# 测试乘法
RESULT=$(cargo run --release -- --a 10 --b 5 --op mul 2>&1)
EXPECTED="Result: 10 mul 5 = 50"
if [[ "$RESULT" == "$EXPECTED" ]]; then
    echo "✅ 乘法测试通过: $RESULT"
else
    echo "❌ 乘法测试失败"
    echo "   期望: $EXPECTED"
    echo "   实际: $RESULT"
    exit 1
fi

# 测试除零错误处理
RESULT=$(cargo run --release -- --a 10 --b 0 --op div 2>&1)
EXPECTED="Error: Division by zero"
if [[ "$RESULT" == "$EXPECTED" ]]; then
    echo "✅ 除零错误处理测试通过: $RESULT"
else
    echo "❌ 除零错误处理测试失败"
    echo "   期望: $EXPECTED"
    echo "   实际: $RESULT"
    exit 1
fi

# 测试 --help 功能
echo "🧪 测试 --help 功能..."
RESULT=$(cargo run --release -- --help 2>&1)
if [[ "$RESULT" =~ "calculator" && "$RESULT" =~ "Simple calculator CLI" ]]; then
    echo "✅ --help 功能测试通过"
else
    echo "❌ --help 功能测试失败"
    exit 1
fi

# 清理测试目录
cd ..
rm -rf "$TEST_DIR"

echo ""
echo "🎉 CLI Basic Demo 所有测试通过！"
echo ""
echo "📊 测试总结:"
echo "  - 项目初始化: ✅"
echo "  - 依赖添加: ✅"
echo "  - 代码编写: ✅"
echo "  - 编译成功: ✅"
echo "  - 功能测试: ✅"
echo "  - 错误处理: ✅"
echo "  - Help 功能: ✅"
echo ""
echo "✅ Demo 流程验证成功！"