# 设计方案: TUI 用户体验增强

日期: 2026-05-19

## 核心目标

- 用户可自定义主题颜色，支持深色/浅色/高对比度预设
- 用户可自定义快捷键映射，所有操作支持重新绑定
- 代码块语法高亮，提升代码可读性
- 自动检测终端能力，智能降级渲染

## 架构设计

### 模块结构

```
matrixcode-tui/src/
├── theme/
│   ├── mod.rs           # Theme trait + 预设主题
│   ├── config.rs        # 主题配置加载 (theme.toml)
│   └── presets.rs       # Dark/Light/HighContrast 预设
│
├── keybindings/
│   ├── mod.rs           # KeyBindings struct + Action enum
│   ├── config.rs        # 快捷键配置加载 (keybindings.toml)
│   └── default.rs       # 默认快捷键映射
│
├── syntax/
│   ├── mod.rs           # 语法高亮模块入口
│   └── highlight.rs     # syntect 集成
│
├── terminal/
│   ├── mod.rs           # 终端能力检测模块
│   ├── detect.rs        # 颜色/Unicode/鼠标检测
│   └── fallback.rs      # 降级渲染策略
│
└── draw.rs              # 改用 Theme.colors 替代硬编码
└── input.rs             # 改用 KeyBindings 替代硬编码
└── markdown.rs          # 调用 syntax.highlight() 替代当前渲染
```

### 配置文件位置

- `~/.matrix/theme.toml` - 主题颜色配置
- `~/.matrix/keybindings.toml` - 快捷键映射配置

## 主题系统设计

### Theme struct

```rust
pub struct Theme {
    pub name: String,

    // 基础颜色
    pub primary: Color,        // 主色调 ( Cyan )
    pub secondary: Color,      // 次色调
    pub background: Color,     // 背景 ( DarkGray )
    pub foreground: Color,     // 前景/文本

    // 状态颜色
    pub success: Color,        // 成功/完成
    pub warning: Color,        // 警告/审批
    pub error: Color,          // 错误
    pub info: Color,           // 信息

    // 角色颜色 (消息渲染)
    pub user_message: Color,
    pub assistant_message: Color,
    pub thinking: Color,
    pub system: Color,

    // UI 元素
    pub border: Color,
    pub cursor: Color,
    pub prompt: Color,
    pub selection_bg: Color,

    // 代码块颜色
    pub code_background: Color,
    pub code_text: Color,
    pub inline_code: Color,
}
```

### 配置文件格式 (theme.toml)

```toml
name = "matrix-dark"

[base]
primary = "#00FFFF"       # Cyan
secondary = "#808080"     # Gray
background = "#1E1E1E"    # DarkGray
foreground = "#FFFFFF"    # White

[status]
success = "#00FF00"       # Green
warning = "#FFFF00"       # Yellow
error = "#FF0000"         # Red
info = "#00BFFF"          # DeepSkyBlue

[roles]
user = "#00FF00"          # Green (用户消息边框)
assistant = "#FFFFFF"     # White
thinking = "#808080"      # DarkGray
system = "#808080"        # DarkGray

[ui]
border = "#00FFFF"
cursor = "#00FFFF"
prompt = "#FFFF00"
selection = "#1E1E1E"
```

## 快捷键系统设计

### Action enum + KeyBindings struct

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // 基础操作
    SendInput,
    Newline,
    Interrupt,
    Exit,
    Paste,

    // 滚动
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    ScrollTop,
    ScrollBottom,

    // 模式切换
    ToggleApproveMode,
    ToggleThinkingCollapse,

    // 输入编辑
    Backspace,
    Delete,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
    HistoryUp,
    HistoryDown,

    // 复制
    CopySelection,
}

pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

pub struct KeyBindings {
    bindings: HashMap<Action, Vec<KeyBinding>>,
}
```

### 配置文件格式 (keybindings.toml)

```toml
# 基础操作
[base]
send = "Enter"
newline = "Shift+Enter"
interrupt = "Escape"
exit = "Ctrl+D"
paste = "Ctrl+V"

# 滚动
[scroll]
up = "Alt+Up"
down = "Alt+Down"
page_up = "PageUp"
page_down = "PageDown"
top = "Home"
bottom = "End"

# 模式切换
[mode]
toggle_approve = "Alt+M"
toggle_thinking = "Alt+T"

# 编辑
[edit]
backspace = "Backspace"
delete = "Delete"
left = "Left"
right = "Right"
home = "Home"
end = "End"
history_up = "Up"
history_down = "Down"

# 复制
copy = "Ctrl+C"
```

## 语法高亮设计

### syntect 集成

```rust
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Highlighter, HighlightIterator};

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
    pub fn new(theme: &Theme) -> Self {
        // 加载内置语法集: Rust, Python, JS, TS, Go, Java, C/C++, etc.
    }

    pub fn highlight(&self, code: &str, lang: &str) -> Vec<Span<'static>> {
        // 1. 解析语法
        // 2. 应用高亮主题
        // 3. 转换为 ratatui Span
    }
}
```

### 依赖添加

```toml
syntect = "5.2"
```

### 与 markdown.rs 集成

- 修改 `render_markdown()` 检测代码块语言
- 调用 `SyntaxHighlighter::highlight()` 替代当前单色渲染
- 未知语言时 fallback 到 `code_block_style()`

## 终端能力检测设计

### TerminalCapabilities struct

```rust
pub struct TerminalCapabilities {
    pub color_support: ColorSupport,
    pub unicode_support: UnicodeSupport,
    pub mouse_support: MouseSupport,
    pub detected_via: DetectionMethod,
}

pub enum ColorSupport {
    TrueColor,   // 24-bit RGB
    TwoFiftySix, // 256-color palette
    Sixteen,     // Basic ANSI colors
    None,
}

pub enum UnicodeSupport {
    Full,        // 所有 Unicode 字符
    Partial,     // 基本Unicode，Emoji可能有问题
    AsciiOnly,   // 仅 ASCII
}

pub enum MouseSupport {
    Full,        // 点击、拖拽、滚动
    ScrollOnly,  // 仅滚动
    None,
}
```

### 检测方法

```rust
impl TerminalCapabilities {
    pub fn detect() -> Self {
        // 1. 检查环境变量: $TERM, $COLORTERM, $TERM_PROGRAM
        // 2. 查询 terminfo 数据库
        // 3. 尝试发送查询序列
        // 4. 启发式判断 (Windows Terminal, iTerm2, etc.)
    }
}
```

### 降级渲染策略

```rust
pub struct FallbackRenderer {
    capabilities: TerminalCapabilities,
    theme: Theme,
}

impl FallbackRenderer {
    pub fn adjust_color(&self, color: Color) -> Color {
        match self.capabilities.color_support {
            ColorSupport::TrueColor => color,
            ColorSupport::TwoFiftySix => color.to_256(),
            ColorSupport::Sixteen => color.to_ansi(),
            ColorSupport::None => Color::Reset,
        }
    }

    pub fn adjust_symbol(&self, symbol: &str) -> String {
        match self.capabilities.unicode_support {
            UnicodeSupport::Full => symbol.to_string(),
            UnicodeSupport::Partial => symbol.replace_emoji(),
            UnicodeSupport::AsciiOnly => symbol.to_ascii_fallback(),
        }
    }
}
```

## 文件变更清单

### 新增文件

| 文件 | 功能 |
|------|------|
| `theme/mod.rs` | Theme trait + 预设主题 |
| `theme/config.rs` | 主题配置加载 |
| `theme/presets.rs` | Dark/Light/HighContrast 预设 |
| `keybindings/mod.rs` | KeyBindings struct + Action enum |
| `keybindings/config.rs` | 快捷键配置加载 |
| `keybindings/default.rs` | 默认快捷键映射 |
| `syntax/mod.rs` | 语法高亮模块入口 |
| `syntax/highlight.rs` | syntect 集成 |
| `terminal/mod.rs` | 终端能力检测模块入口 |
| `terminal/detect.rs` | 颜色/Unicode/鼠标检测 |
| `terminal/fallback.rs` | 降级渲染策略 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `draw.rs` | 使用 Theme 替代硬编码颜色 |
| `input.rs` | 使用 KeyBindings 替代硬编码快捷键 |
| `markdown.rs` | 调用 SyntaxHighlighter |
| `lib.rs` | 初始化 Theme, KeyBindings, TerminalCapabilities |
| `app.rs` | 添加 theme, keybindings, terminal 字段 |
| `Cargo.toml` | 添加 syntect 依赖 |

## 验收标准

- [ ] 主题配置文件 `~/.matrix/theme.toml` 加载成功
- [ ] 颜色应用正确，draw.rs 使用 Theme 颜色
- [ ] 快捷键配置文件 `~/.matrix/keybindings.toml` 加载成功
- [ ] 所有 Action 可自定义快捷键
- [ ] 代码块语法高亮支持主流语言 (Rust, Python, JS, TS, Go)
- [ ] 终端检测准确判断颜色/Unicode/鼠标支持
- [ ] 降级渲染正确处理低能力终端

## 风险与应对

| 风险 | 应对策略 |
|------|----------|
| syntect 编译时间长 | 使用 `syntect::defaults` 内置语法集，避免自定义编译 |
| 终端检测不准确 | 多方法检测 + 优先级排序 + 手动覆盖配置 |
| 配置文件格式错误 | 宽松解析 + fallback 到默认值 + 错误提示 |

## 后续扩展

- Core 模块优化：agent.rs 拆分 + Hook 机制
- 更多预设主题
- 自定义 syntax 高亮主题
- 终端能力手动覆盖配置