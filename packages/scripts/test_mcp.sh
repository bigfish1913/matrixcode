#!/bin/bash
# MCP 功能测试脚本

set -e

echo "=== MCP 核心功能测试 ==="
echo ""

# 测试 1: 基本启动（无 MCP）
echo "测试 1: 基本启动（无 MCP）..."
cargo run --package matrixcode-tui -- --help > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ 基本启动成功"
else
    echo "❌ 基本启动失败"
    exit 1
fi

# 测试 2: CLI 参数解析
echo ""
echo "测试 2: CLI 参数解析..."
cargo run --package matrixcode-tui -- --mcp "test:npx -y test-server" --help > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "✅ CLI 参数解析成功"
else
    echo "❌ CLI 参数解析失败"
    exit 1
fi

# 测试 3: 多 MCP 参数
echo ""
echo "测试 3: 多 MCP 参数..."
cargo run --package matrixcode-tui -- \
  --mcp "playwright:npx -y @playwright/mcp@latest" \
  --mcp "filesystem:npx -y @modelcontextprotocol/server-filesystem" \
  --help > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "✅ 多 MCP 参数解析成功"
else
    echo "❌ 多 MCP 参数解析失败"
    exit 1
fi

echo ""
echo "=== 所有测试通过 ==="
echo ""
echo "注意：实际 MCP server 连接测试需要手动验证。"
echo "请运行以下命令进行完整测试："
echo ""
echo "  matrixcode-tui --mcp \"playwright:npx -y @playwright/mcp@latest\""
echo ""
echo "然后在 Agent 对话中询问："
echo "  - \"列出当前的 MCP servers\""
echo "  - \"测试 Playwright 工具\""