# FocusTracker 硬编码消除 - 优化总结

## 🎯 问题识别

在审查 `FocusTracker` 代码时发现多处硬编码：

### 硬编码位置

```rust
// ❌ 原代码中的硬编码

// 硬编码关键词列表
transition_keywords: vec![
    "however".to_string(), "but".to_string(), ...
],
question_keywords: vec![
    "how".to_string(), "what".to_string(), ...
],
task_keywords: vec![
    "implement".to_string(), "create".to_string(), ...
],

// 硬编码窗口大小
pub fn detect_focus(&self, messages: &[Message], window_size: usize) // 需要手动传入

// 硬编码限制值
if focus.recent_context.len() >= 5 { break; }  // 硬编码 5

// 硬编码文本长度阈值
.filter(|s| s.trim().len() > 10)  // 硬编码 10
if text.len() > 20 { ... }        // 硬编码 20

// 硬编码提取长度
text.chars().take(100)  // 硬编码 100 chars
```

**问题影响：**
- ✗ 不可配置：无法根据对话复杂度动态调整
- ✗ 不灵活：无法针对不同场景优化
- ✗ 维护困难：修改需要改动多处代码

---

## ✅ 解决方案

### 1. 创建 FocusTrackerConfig 模块

**新文件：** `core/src/compress/focus_config.rs`

```rust
pub struct FocusTrackerConfig {
    // 关键词列表（可配置）
    pub transition_keywords: Vec<String>,
    pub question_keywords: Vec<String>,
    pub task_keywords: Vec<String>,
    
    // 窗口大小和限制（可配置）
    pub focus_window_size: usize,              // 默认 10
    pub max_recent_context_count: usize,       // 默认 5
    pub max_question_extract_length: usize,    // 默认 100
    pub min_substantial_text_length: usize,    // 默认 10
    
    // 评分参数（可配置）
    pub focus_score_boost: f32,                // 默认 0.3
    pub max_focus_score: f32,                  // 默认 1.0
}
```

### 2. 提供多种配置预设

```rust
impl FocusTrackerConfig {
    // 默认配置（平衡）
    pub fn default() -> Self {
        Self {
            focus_window_size: 10,
            max_recent_context_count: 5,
            ...
        }
    }
    
    // 简单对话配置（更激进压缩）
    pub fn simple_conversation() -> Self {
        Self {
            focus_window_size: 5,
            max_recent_context_count: 3,
            min_substantial_text_length: 5,
            ...
        }
    }
    
    // 复杂技术讨论配置（更保守压缩）
    pub fn complex_technical() -> Self {
        Self {
            focus_window_size: 15,
            max_recent_context_count: 7,
            max_question_extract_length: 150,
            min_substantial_text_length: 20,
            focus_score_boost: 0.4,
            ...
        }
    }
    
    // 从复杂度级别自动选择配置
    pub fn from_complexity(level: ComplexityLevel) -> Self {
        match level {
            ComplexityLevel::High => Self::complex_technical(),
            ComplexityLevel::Medium => Self::default(),
            ComplexityLevel::Low => Self::simple_conversation(),
        }
    }
}
```

### 3. 重构 FocusTracker

```rust
pub struct FocusTracker {
    config: FocusTrackerConfig,  // 替代硬编码
}

impl FocusTracker {
    // 使用默认配置
    pub fn new() -> Self {
        Self {
            config: FocusTrackerConfig::default(),
        }
    }
    
    // 使用自定义配置
    pub fn with_config(config: FocusTrackerConfig) -> Self {
        Self { config }
    }
    
    // API 简化：不再需要手动传入 window_size
    pub fn detect_focus(&self, messages: &[Message]) -> ConversationFocus {
        self.detect_focus_with_window(messages, self.config.focus_window_size)
    }
    
    // 使用配置替代硬编码值
    fn extract_key_point(&self, message: &Message) -> Option<String> {
        let sentences: Vec<&str> = text.split(...)
            .filter(|s| s.trim().len() > self.config.min_substantial_text_length)
            .collect();
        
        if focus.recent_context.len() >= self.config.max_recent_context_count {
            break;
        }
    }
    
    fn extract_current_question(&self, message: &Message) -> Option<String> {
        text.chars()
            .take(self.config.max_question_extract_length)
            .collect::<String>()
    }
}
```

---

## 📊 改进效果

### 代码质量改进

| 改进项 | 原状态 | 新状态 |
|--------|--------|--------|
| 硬编码关键词 | 3 处硬编码列表 | ✅ 配置化 |
| 硬编码窗口大小 | 手动传参 | ✅ 配置化默认值 |
| 硬编码限制值 | 5 处硬编码数字 | ✅ 配置化参数 |
| 配置灵活性 | 无法调整 | ✅ 3 种预设 + 自定义 |
| 自适应能力 | 无 | ✅ 根据复杂度自动选择 |

### 配置对比

| 场景 | 窗口大小 | 上下文数量 | 提取长度 | 最小文本长度 |
|------|----------|------------|----------|--------------|
| **简单对话** | 5 | 3 | 100 | 5 |
| **默认（平衡）** | 10 | 5 | 100 | 10 |
| **复杂技术** | 15 | 7 | 150 | 20 |

**效果：**
- ✅ 简单对话：更激进压缩，节省 token
- ✅ 复杂讨论：更保守压缩，保留更多上下文
- ✅ 自适应：根据 ComplexityAnalyzer 自动选择

---

## 🔧 使用示例

### 1. 默认使用（最简单）

```rust
let tracker = FocusTracker::new();
let focus = tracker.detect_focus(&messages);
```

### 2. 根据复���度自动配置

```rust
let complexity = ComplexityAnalyzer::analyze(&messages);
let config = FocusTrackerConfig::from_complexity(complexity);
let tracker = FocusTracker::with_config(config);
let focus = tracker.detect_focus(&messages);
```

### 3. 自定义配置

```rust
let config = FocusTrackerConfig::default()
    .with_custom_keywords(KeywordType::Task, vec!["optimize".to_string()]);

let tracker = FocusTracker::with_config(config);
```

### 4. 手动指定配置

```rust
let config = FocusTrackerConfig {
    focus_window_size: 20,
    max_recent_context_count: 10,
    focus_score_boost: 0.5,
    ...Default::default()
};

let tracker = FocusTracker::with_config(config);
```

---

## 🎨 扩展性改进

### 支持自定义关键词

```rust
pub enum KeywordType {
    Transition,  // 话题转换关键词
    Question,    // 问题关键词
    Task,        // 任务关键词
}

// 添加领域特定关键词
let config = FocusTrackerConfig::default()
    .with_custom_keywords(KeywordType::Task, vec![
        "优化".to_string(),
        "重构".to_string(),
        "debug".to_string(),
    ]);
```

### 配置验证

```rust
pub fn validate(&self) -> bool {
    self.focus_window_size > 0 &&
    self.max_recent_context_count > 0 &&
    self.max_question_extract_length > 0 &&
    self.focus_score_boost > 0.0 &&
    self.max_focus_score > 0.0
}

// 使用前验证
if !config.validate() {
    panic!("Invalid FocusTrackerConfig");
}
```

---

## ✅ 测试验证

所有测试通过：

```bash
test compress::focus::tests::test_focus_tracker_creation ... ok
test compress::focus::tests::test_focus_tracker_with_custom_config ... ok
test compress::focus::tests::test_detect_focus ... ok
test compress::focus::tests::test_focus_score ... ok
test compress::focus_config::tests::test_default_config ... ok
test compress::focus_config::tests::test_simple_conversation_config ... ok
test compress::focus_config::tests::test_complex_technical_config ... ok
test compress::focus_config::tests::test_with_custom_keywords ... ok
test compress::integration::tests::test_focus_message_injection ... ok
```

**覆盖率：**
- ✅ 配置创建测试
- ✅ 预设配置测试
- ✅ 自定义关键词测试
- ✅ 焦点检测测试
- ✅ 焦点评分测试
- ✅ 焦点消息注入测试

---

## 📝 API 变更说明

### API 简化

**原 API（需要手动传参）：**
```rust
let focus = tracker.detect_focus(&messages, 10);  // ❌ 需要手动指定窗口大小
```

**新 API（自动使用配置）：**
```rust
let focus = tracker.detect_focus(&messages);  // ✅ 使用配置的默认窗口大小
```

### 新增方法

```rust
// 获取配置引用
pub fn config(&self) -> &FocusTrackerConfig

// 带自定义窗口大小的检测（可选）
pub fn detect_focus_with_window(&self, messages: &[Message], window_size: usize)

// 创建焦点消息（用于注入到压缩后的对话）
pub fn create_focus_message(&self, focus: &ConversationFocus) -> Message
```

---

## 🚀 后续优化建议

### 1. 持久化配置

```rust
// 从文件加载配置
pub fn load_from_file(path: &str) -> Result<Self> {
    let content = fs::read_to_string(path)?;
    let config: FocusTrackerConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

// 保存配置到文件
pub fn save_to_file(&self, path: &str) -> Result<()> {
    let content = serde_yaml::to_string(self)?;
    fs::write(path, content)?;
    Ok(())
}
```

**配置文件示例（focus_config.yaml）：**
```yaml
focus_window_size: 12
max_recent_context_count: 6
focus_score_boost: 0.35
transition_keywords:
  - "however"
  - "转换"
  - "切换"
task_keywords:
  - "optimize"
  - "重构"
```

### 2. 动态调整配置

```rust
// 根据运行时反馈调整配置
pub fn adjust_based_on_feedback(&mut self, feedback: &CompressionFeedback) {
    if feedback.focus_accuracy < 0.7 {
        // 焦点准确性低 → 增加窗口大小
        self.focus_window_size += 2;
        self.max_recent_context_count += 1;
    }
    
    if feedback.token_usage_rate > 0.85 {
        // Token 使用率高 → 更激进压缩
        self.max_question_extract_length -= 20;
        self.min_substantial_text_length += 5;
    }
}
```

### 3. 语言检测自适应

```rust
// 根据对话语言自动选择关键词
pub fn from_language(messages: &[Message]) -> Self {
    let language = detect_language(messages);
    
    match language {
        Language::Chinese => Self {
            transition_keywords: vec!["转换", "切换", "换个话题", ...],
            question_keywords: vec!["如何", "什么", "为什么", ...],
            task_keywords: vec!["实现", "创建", "修复", ...],
            ...Default::default()
        },
        Language::English => Self::default(),
        Language::Mixed => Self::default(),  // 双语关键词
    }
}
```

---

## 📌 总结

### 核心改进

1. **消除硬编码** ✅
   - 关键词列表 → 配置化
   - 窗口大小 → 配置化
   - 限制值 → 配置化参数

2. **提升灵活性** ✅
   - 3 种预设配置（简单/默认/复杂）
   - 自定义配置支持
   - 自适应复杂度

3. **改善可维护性** ✅
   - 配置集中管理
   - 单一修改点
   - 配置验证机制

4. **增强扩展性** ✅
   - 自定义关键词添加
   - 配置持久化（建议）
   - 动态调整（建议）

### 关键优势

- **自适应：** 根据对话复杂度自动优化参数
- **可配置：** 支持多种场景的最佳配置
- **易维护：** 配置集中，修改简单
- **易扩展：** 支持自定义关键词和动态调整

---

**文档版本：** v1.0  
**最后更新：** 2025-01-XX  
**修改文件：**
- `core/src/compress/focus_config.rs` (新增)
- `core/src/compress/focus.rs` (重构)
- `core/src/compress/integration.rs` (API 更新)
- `core/src/compress/mod.rs` (导出新模块)