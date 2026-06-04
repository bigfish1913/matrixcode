#!/bin/bash

# LSP Test Script
# 测试 LSP 功能的集成

echo "=== LSP Integration Test ==="
echo ""

# 1. 测试 LSP 配置加载
echo "1. Testing LSP config loading..."
if [ -f "lsp.toml" ]; then
    echo "✓ lsp.toml found"
    cat lsp.toml | grep "command = \"rust-analyzer\"" && echo "✓ rust-analyzer configured"
else
    echo "✗ lsp.toml not found"
fi
echo ""

# 2. 测试 LSP Manager 功能
echo "2. Testing LSP Manager..."
cargo test --package matrixcode-core manager_test --lib 2>&1 | grep "test result:"
echo ""

# 3. 测试 LSP 工具定义
echo "3. Testing LSP Tools..."
cargo test --package matrixcode-core test_lsp --lib 2>&1 | grep "test result:"
echo ""

# 4. 检查 TUI 构建
echo "4. Checking TUI build..."
cargo build --release --package matrixcode-tui 2>&1 | tail -5
echo ""

# 5. 测试 rust-analyzer 可用性（可选）
echo "5. Checking rust-analyzer availability..."
if command -v rust-analyzer &> /dev/null; then
    echo "✓ rust-analyzer is installed"
    rust-analyzer --version
else
    echo "✗ rust-analyzer not found (optional for full LSP test)"
fi
echo ""

echo "=== Test Complete ==="