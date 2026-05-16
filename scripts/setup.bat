@echo off
REM Development setup script for MatrixCode monorepo (Windows)

echo 🚀 Setting up MatrixCode development environment...

REM Check prerequisites
echo Checking prerequisites...

where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Rust/Cargo not installed. Please install from https://rustup.rs/
    exit /b 1
)
echo ✅ Cargo found

where node >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Node.js not installed. Please install from https://nodejs.org/
    exit /b 1
)
echo ✅ Node.js found

REM Setup CLI
echo.
echo 📦 Setting up CLI...
cd packages\cli

if not exist ".env" (
    echo Creating .env from .env.example...
    copy .env.example .env
    echo ⚠️  Please edit packages\cli\.env to add your API key
)

echo Building CLI...
cargo build --release
echo ✅ CLI built successfully

REM Setup VSCode extension
echo.
echo 📦 Setting up VSCode extension...
cd ..\vscode

echo Installing npm dependencies...
call npm install

echo Building extension...
call npm run compile
echo ✅ VSCode extension built successfully

REM Back to root
cd ..\..

echo.
echo ✨ Setup complete!
echo.
echo Next steps:
echo   1. Edit packages\cli\.env to add your API key
echo   2. Run CLI: cd packages\cli && cargo run --release
echo   3. Debug VSCode extension: Open VSCode, press F5
echo.
echo Useful commands:
echo   task build          - Build CLI
echo   task build-vscode   - Build VSCode extension
echo   task test           - Run CLI tests
echo   task clean          - Clean build artifacts