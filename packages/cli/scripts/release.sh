#!/bin/bash
# MatrixCode 发布脚本

set -e

cd packages/cli
VERSION=$(grep "^version = " crates/matrixcode-cli/Cargo.toml | cut -d'"' -f2)
echo "当前版本: $VERSION"

echo ""
echo "发布步骤:"
echo "1. cargo test           - 运行测试"
echo "2. cargo build --release - 构建发布版本"
echo "3. cargo package         - 检查package"
echo "4. cargo publish         - 发布到crates.io"
echo "5. npm publish           - 发布到npm"
echo ""

read -p "继续发布? (y/n): " confirm
if [[ "$confirm" != "y" ]]; then
    echo "已取消"
    exit 0
fi

echo ""
echo "=== 1. 运行测试 ==="
cargo test --all

echo ""
echo "=== 2. 构建发布版本 ==="
cargo build --release

echo ""
echo "=== 3. 检查package ==="
cargo package --list

echo ""
echo "=== 4. 发布到crates.io ==="
echo "提示: 需要先登录 cargo login <token>"
cargo publish -p matrixcode-core --dry-run
cargo publish -p matrixcode-tui --dry-run  
cargo publish -p matrixcode --dry-run

echo ""
echo "=== 5. 发布到npm ==="
cd npm
npm publish --dry-run

echo ""
echo "✅ 检查完成！去除 --dry-run 参数执行实际发布"
