#!/bin/bash
# Playwright MCP 测试脚本

echo "=== 测试 Playwright MCP 连接 ==="
echo ""

# 测试 Playwright MCP 是否能正常启动
echo "1. 检查 npx 是否可用..."
if command -v npx &> /dev/null; then
    echo "✅ npx 可用"
else
    echo "❌ npx 不可用，请安装 Node.js"
    exit 1
fi

echo ""
echo "2. 测试 Playwright MCP 启动..."
timeout 10s npx -y @playwright/mcp@latest 2>&1 | head -20 || echo "⚠️  启动超时（正常，MCP server 需要客户端连接）"

echo ""
echo "=== 测试完成 ==="
echo ""
echo "现在你可以运行以下命令测试打开百度："
echo ""
echo "  matrixcode-tui --mcp \"playwright:npx -y @playwright/mcp@latest\""
echo ""
echo "然后在对话中输入："
echo "  \"使用 Playwright 打开百度 https://www.baidu.com\""
echo ""
echo "Agent 会调用 Playwright MCP 的 browser_navigate 工具打开浏览器。"