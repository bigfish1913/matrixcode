# 测试方案 B - 粘贴后一次 Enter 就发送

## 修改说明
我将实现方案 B：
- 粘贴多行文本 → 立即折叠显示
- 自动触发的 Enter（从粘贴换行符）被过滤
- 用户手动按 Enter → 直接发送（不需要二次确认）
- 手动输入多行仍需要两次 Enter 确认

## 请帮我手动修改代码
由于换行符问题导致自动编辑失败，请你手动修改以下内容：

### 文件：tui/src/input.rs

**修改位置 1：第 37-38 行**
```rust
// 修改前：
log::debug!("Enter key pressed: input_collapsed={}, multiline_confirm_send={}, input_len={}, has_newline={}", 
    self.input_collapsed, self.multiline_confirm_send, self.input.len(), self.input.contains('\n'));

// 修改后：
log::info!("🔑 Enter key pressed: input_len={}, has_newline={}", 
    self.input.len(), self.input.contains('\n'));
```

**修改位置 2：第 45-47 行**
```rust
// 修改前：
self.multiline_confirm_send = false; // Reset confirmation state
self.input_collapsed = false; // Expand input when manually adding newlines
log::debug!("Shift+Enter: inserted newline, input_collapsed=false");

// 修改后：
self.input_collapsed = false; // Expand input when manually adding newlines
log::info!("✓ Shift+Enter: inserted newline, expanded input");
```

**修改位置 3：第 56-83 行（核心修改）**
```rust
// 修改前：
} else if !self.input.trim().is_empty() {
    // Check if input is collapsed (from paste) - first Enter should expand it
    if self.input_collapsed && self.input.contains('\n') {
        // First Enter after paste: expand input, don't send
        self.input_collapsed = false;
        self.multiline_confirm_send = true; // Mark for confirmation
        log::info!("✓ First Enter after paste: expanding input, NOT sending");
        self.push_message(Message {
            role: Role::System,
            content: "⚠️ 多行内容已展开，按 Enter 再次确认发送".into(),
            is_pending: false,
        });
    } else if self.input.contains('\n') && !self.multiline_confirm_send {
        // First Enter on manually typed multiline: require confirmation
        self.multiline_confirm_send = true;
        log::info!("✓ First Enter on multiline: marking for confirmation, NOT sending");
        self.push_message(Message {
            role: Role::System,
            content: "⚠️ 多行内容，按 Enter 再次确认发送".into(),
            is_pending: false,
        });
    } else {
        // Second Enter on multiline, or single line: send
        log::info!("✓ Sending input (second Enter or single line)");
        self.multiline_confirm_send = false;
        self.input_collapsed = false;
        self.send_input();
    }

// 修改后：
} else if !self.input.trim().is_empty() {
    // SCHEME B: Single Enter sends pasted content (no double confirmation)
    // But manual multiline still needs confirmation
    
    if self.input_collapsed && self.input.contains('\n') {
        // Paste multiline: send directly on first manual Enter
        log::info!("✓ Enter after paste: sending directly (scheme B)");
        self.input_collapsed = false;
        self.send_input();
    } else if self.input.contains('\n') && !self.multiline_confirm_send {
        // Manually typed multiline: first Enter asks for confirmation
        self.multiline_confirm_send = true;
        log::info!("✓ First Enter on manual multiline: marking for confirmation");
        self.push_message(Message {
            role: Role::System,
            content: "⚠️ 多行内容，按 Enter 再次确认发送".into(),
            is_pending: false,
        });
    } else {
        // Second Enter on manual multiline, or single line: send
        log::info!("✓ Sending input");
        self.multiline_confirm_send = false;
        self.input_collapsed = false;
        self.send_input();
    }
```

## 关键变化
1. **粘贴多行**：`input_collapsed && has_newline` → 直接 `send_input()`
2. **手动多行**：仍需要两次 Enter 确认（安全）
3. **日志级别提升**：从 `debug` → `info`，确保能看到

## 测试方法
```bash
set RUST_LOG=info
./target/release/matrixcode-tui.exe
```

粘贴多行后：
- 应看到：`🔑 Enter key pressed`
- 应看到：`✓ Enter after paste: sending directly (scheme B)`
- 应看到：`🚀 send_input called`

如果还是看不到日志，可能是终端不支持 Bracketed Paste 或其他问题。