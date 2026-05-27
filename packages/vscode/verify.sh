#!/bin/bash

echo "========================================"
echo "MatrixCode Extension Verification"
echo "========================================"
echo ""

# 检查当前目录
if [ ! -f "package.json" ]; then
    echo "❌ Error: Not in packages/vscode directory!"
    echo "Please run: cd packages/vscode"
    exit 1
fi

echo "✅ Correct directory"
echo ""

# 检查 dist 目录
echo "1. Checking dist directory..."
if [ -f "dist/extension.js" ]; then
    size=$(stat -f%z "dist/extension.js" 2>/dev/null || stat --printf="%s" "dist/extension.js" 2>/dev/null)
    echo "✅ dist/extension.js exists (${size} bytes)"
else
    echo "❌ dist/extension.js missing - run: npm run compile-dev"
fi

if [ -f "dist/extension.js.map" ]; then
    echo "✅ dist/extension.js.map exists (sourcemap)"
else
    echo "⚠️ dist/extension.js.map missing (debugging won't work properly)"
fi
echo ""

# 检查 resources 目录
echo "2. Checking resources directory..."
if [ -f "resources/icon.svg" ]; then
    echo "✅ resources/icon.svg exists"
else
    echo "❌ resources/icon.svg missing"
fi
echo ""

# 检查 package.json
echo "3. Checking package.json..."
main=$(cat package.json | grep '"main"' | head -1)
if [[ "$main" == *"./dist/extension.js"* ]]; then
    echo "✅ main entry point correct"
else
    echo "❌ main entry point incorrect: $main"
fi

activation=$(cat package.json | grep "onStartupFinished")
if [[ "$activation" == *"onStartupFinished"* ]]; then
    echo "✅ activationEvents configured"
else
    echo "❌ activationEvents missing"
fi

viewsContainer=$(cat package.json | grep '"id": "matrixcode"' | head -1)
if [[ "$viewsContainer" == *"matrixcode"* ]]; then
    echo "✅ viewsContainers configured"
else
    echo "❌ viewsContainers missing"
fi
echo ""

# 编译
echo "4. Compiling development version..."
npm run compile-dev
echo ""

echo "========================================"
echo "Verification Complete!"
echo "========================================"
echo ""
echo "Next steps:"
echo "1. In VSCode (this directory), press F5"
echo "2. Select 'Run Extension (Debug)'"
echo "3. In the new window:"
echo "   - Press Ctrl+Shift+U"
echo "   - Select 'MatrixCode' in output dropdown"
echo "   - Check for activation logs"
echo ""
echo "Expected output logs:"
echo "  'MatrixCode extension is activating...'"
echo "  'StatusBar item added'"
echo "  'MatrixCode extension activated successfully!'"
echo ""
echo "If you see 🤖 MatrixCode in the status bar, it works!"