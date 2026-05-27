@echo off
echo ========================================
echo MatrixCode Extension Verification
echo ========================================
echo.

REM 检查当前目录
if not exist package.json (
    echo ❌ Error: Not in packages\vscode directory!
    echo Please run: cd packages\vscode
    exit /b 1
)

echo ✅ Correct directory
echo.

REM 检查 dist 目录
echo 1. Checking dist directory...
if exist dist\extension.js (
    echo ✅ dist\extension.js exists
) else (
    echo ❌ dist\extension.js missing - run: npm run compile-dev
)

if exist dist\extension.js.map (
    echo ✅ dist\extension.js.map exists (sourcemap)
) else (
    echo ⚠️ dist\extension.js.map missing (debugging won't work properly)
)
echo.

REM 检查 resources 目录
echo 2. Checking resources directory...
if exist resources\icon.svg (
    echo ✅ resources\icon.svg exists
) else (
    echo ❌ resources\icon.svg missing
)
echo.

REM 检查 package.json
echo 3. Checking package.json...
findstr /C:"\"main\": \"./dist/extension.js\"" package.json >nul
if %errorlevel%==0 (
    echo ✅ main entry point correct
) else (
    echo ❌ main entry point incorrect
)

findstr /C:"onStartupFinished" package.json >nul
if %errorlevel%==0 (
    echo ✅ activationEvents configured
) else (
    echo ❌ activationEvents missing
)
echo.

REM 编译
echo 4. Compiling development version...
call npm run compile-dev
echo.

echo ========================================
echo Verification Complete!
echo ========================================
echo.
echo Next steps:
echo 1. In VSCode (this directory), press F5
echo 2. Select 'Run Extension (Debug)'
echo 3. In the new window:
echo    - Press Ctrl+Shift+U
echo    - Select 'MatrixCode' in output dropdown
echo    - Check for activation logs
echo.
echo Expected output logs:
echo   'MatrixCode extension is activating...'
echo   'StatusBar item added'
echo   'MatrixCode extension activated successfully!'
echo.
echo If you see MatrixCode in the status bar, it works!