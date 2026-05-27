#!/bin/bash
# Development setup script for MatrixCode monorepo

set -e

echo "🚀 Setting up MatrixCode development environment..."

# Check prerequisites
echo "Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not installed. Please install from https://rustup.rs/"
    exit 1
fi
echo "✅ Cargo found: $(cargo --version)"

if ! command -v node &> /dev/null; then
    echo "❌ Node.js not installed. Please install from https://nodejs.org/"
    exit 1
fi
echo "✅ Node.js found: $(node --version)"

# Setup CLI
echo ""
echo "📦 Setting up CLI..."
cd packages/cli

if [ ! -f ".env" ]; then
    echo "Creating .env from .env.example..."
    cp .env.example .env
    echo "⚠️  Please edit packages/cli/.env to add your API key"
fi

echo "Building CLI..."
cargo build --release
echo "✅ CLI built successfully"

# Setup VSCode extension
echo ""
echo "📦 Setting up VSCode extension..."
cd ../vscode

echo "Installing npm dependencies..."
npm install

echo "Building extension..."
npm run compile
echo "✅ VSCode extension built successfully"

# Back to root
cd ../..

echo ""
echo "✨ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Edit packages/cli/.env to add your API key"
echo "  2. Run CLI: cd packages/cli && cargo run --release"
echo "  3. Debug VSCode extension: Open VSCode, press F5"
echo ""
echo "Useful commands (using Taskfile):"
echo "  task build          - Build CLI"
echo "  task build-vscode   - Build VSCode extension"
echo "  task test           - Run CLI tests"
echo "  task clean          - Clean build artifacts"
echo "  task --list         - Show all available tasks"