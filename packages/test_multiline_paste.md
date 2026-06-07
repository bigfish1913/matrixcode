# 多行粘贴测试说明

## 测试步骤

### 1. 运行 TUI（带详细日志）
```bash
# Windows CMD
set RUST_LOG=info
./target/release/matrixcode-tui.exe

# PowerShell
$env:RUST_LOG="info"
./target/release/matrixcode-tui.exe
```

### 2. 测试粘贴多行文本

**准备测试内容：**
复制以下多行文本：
```
这是第一行
这是第二行
这是第三行
这是第四行
```

**操作步骤：**
1. 在 TUI 中按 Ctrl+V 粘贴
2. 观察：
   - 输入框是否显示折叠状态？
   - 日志是否显示 "📥 Paste event"？
   - 日志是否显示 "✓ Pre-set collapsed state"？

3. 按 Enter 第一次
   - 观察：
     - 输入框是否展开？
     - 是否显示提示 "⚠️ 多行内容已展开"？
     - 日志是否显示 "✓ First Enter after paste: NOT sending"？

4. 按 Enter 第二次
   - 观察：
     - 是否才真正发送？
     - 日志是否显示 "🚀 send_input called"？

### 3. 查看日志输出

**关键日志标记：**
- `📥 Paste event` - 粘贴事件触发
- `✓ Pre-set collapsed state` - 设置折叠状态
- `✓ First Enter after paste: NOT sending` - 第一次 Enter，不发送
- `🚀 send_input called` - 开始发送消息

**问题诊断：**
如果看到以下日志，说明有问题：
- 粘贴后立即出现 `🚀 send_input called`（粘贴立即发送）
- 第一次 Enter 就出现 `🚀 send_input called`（第一次 Enter 就发送）
- 多次 `🚀 send_input called`（多次发送）

## 预期正确行为

粘贴多行 → 立即折叠显示
第一次 Enter → 展开内容，显示确认提示，不发送
第二次 Enter → 才真正发送

如果不符合预期，请将完整日志输出发给我。
