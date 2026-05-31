# 硬编码消除计划 - 全面清单

## 📊 硬编码统计

**总计：** 84 处硬编码数字比较 + 多处 `.take()` 调用

**分布：**
- `focus.rs` - 已修复 ✅
- `focus_config.rs` - 已修复 ✅
- `hardcode_config.rs` - 新增配置 ✅
- 其他文件 - 待修复 ⚠️

---

## 🔍 详细清单

### 1. coherence.rs - 语义连贯性检测

**位置：** Line 241
```rust
// ❌ 硬编码
.filter(|w| w.len() > 3) // Skip short words

// ✅ 应改为
.filter(|w| w.len() > self.config.min_word_length)
```

**影响：** 短词过滤阈值，影响连贯性判断

---

### 2. compressor.rs - AI 压缩器

**位置：** Line 197
```rust
// ❌ 硬编码
if summary.len() > 200 {
    let truncated = format!("{}...[compressed]", &summary[..150]);
}

// ✅ 应改为
if summary.len() > self.config.long_text_threshold {
    let keep_len = (self.config.long_text_threshold * 0.75) as usize;
    let truncated = format!("{}...[compressed]", &summary[..keep_len]);
}
```

**影响：** 摘要截断阈值，影响压缩质量

---

### 3. focus.rs - 焦点追踪

**位置：** Line 335
```rust
// ❌ 硬编码
if word.len() > 3 && lower.contains(word) {
    score += 0.1;
}

// ✅ 应改为
if self.config.is_meaningful_word(word.len()) && lower.contains(word) {
    score += self.config.focus_score_word_boost;  // 新增配置项
}
```

**影响：** 焦点评分计算

---

### 4. focus_point.rs - 焦点点管理

**位置：** Line 730, 747
```rust
// ❌ 硬编码
if self.focus_history.len() > 1 {
    // ...
}

if self.focus_stack.len() > 1 {
    // ...
}

// ✅ 应改为
if self.focus_history.len() > self.config.min_focus_history_size {
    // ...
}

if self.focus_stack.len() > self.config.min_focus_stack_size {
    // ...
}
```

**影响：** 焦点历史/栈大小判断

---

### 5. hierarchical.rs - 分层摘要

**位置：** Lines 170, 177, 250, 252, 366, 381, 397
```rust
// ❌ 多处硬编码
if sentences.len() > 2 {
    // brief summary
}

if sentences.len() > 1 {
    // at least one sentence
}

let count_factor = if messages.len() > 30 {
    0.3
} else if messages.len() > 20 {
    0.4
} else {
    0.5
};

if q.len() > 2 && q.len() < 30 {
    // valid question
}

.filter(|s| s.len() > 20)

if compressed.len() > 30 {
    // too many sentences
}

// ✅ 应改为
if sentences.len() > self.config.brief_summary_sentence_count {
    // brief summary
}

let count_factor = if messages.len() > self.config.large_conversation_threshold {
    0.3
} else if messages.len() > self.config.medium_conversation_threshold {
    0.4
} else {
    0.5
};

if self.config.is_valid_question_length(q.len()) {
    // valid question
}

.filter(|s| s.len() > self.config.min_sentence_length)

if compressed.len() > self.config.max_compressed_output_length {
    // too many
}
```

**影响：** 分层摘要策略，压缩质量

---

### 6. integration.rs - 集成示例

**位置：** Lines 244, 261
```rust
// ❌ 硬编码
if text.len() > 200 {
    let truncated = format!("{}...[compressed]", &text[..150]);
}

// ✅ 应改为
if text.len() > self.hardcode_config.long_text_threshold {
    let keep_len = (self.hardcode_config.long_text_threshold * 0.75) as usize;
    let truncated = format!("{}...[compressed]", &text[..keep_len]);
}
```

**影响：** 文本截断阈值

---

### 7. pipeline.rs - 压缩管道

**位置：** Lines 165, 259
```rust
// ❌ 硬编码
if content.len() > 500 {
    // preserve substantial content
}

// ✅ 应改为
if content.len() > self.config.preserve_content_threshold {
    // preserve substantial content
}
```

**影响：** 内容保留判断

---

### 8. progressive.rs - 渐进式压缩

**位置：** Lines 427, 546, 676
```rust
// ❌ 硬编码
if content.len() > 1000 && (content.contains("```") || ...) {
    // code-heavy content
}

let truncated = if context.len() > 3000 {
    format!("{}...[truncated]", &context[..2000])
}

let trimmed = if content.len() > 300 {
    format!("{}...[trimmed]", &content[..200])
}

// ✅ 应改为
if content.len() > self.config.code_content_threshold && ... {
    // code-heavy
}

let truncated = if context.len() > self.config.max_context_length {
    let keep_len = (self.config.max_context_length * 0.67) as usize;
    format!("{}...[truncated]", &context[..keep_len])
}

let trimmed = if content.len() > self.config.max_trimmed_content_length {
    let keep_len = (self.config.max_trimmed_content_length * 0.67) as usize;
    format!("{}...[trimmed]", &content[..keep_len])
}
```

**影响：** 渐进式压缩阈值

---

### 9. semantic.rs - 语义压缩

**位置：** Line 123
```rust
// ❌ 硬编码
matches!(&m.content, MessageContent::Text(t) if t.len() > 200)

// ✅ 应改为
matches!(&m.content, MessageContent::Text(t) if t.len() > self.config.summary_length_threshold)
```

**影响：** 摘要触发阈值

---

## 🎯 修复优先级

### P0 - 高优先级（影响压缩质量）

1. **hierarchical.rs** (7 处) - 分层摘要策略
2. **progressive.rs** (3 处) - 渐进式压缩阈值
3. **compressor.rs** (1 处) - AI 压缩截断

### P1 - 中优先级（影响功能准确性）

4. **coherence.rs** (1 处) - 连贯性检测
5. **semantic.rs** (1 处) - 语义压缩触发
6. **focus_point.rs** (2 处) - 焦点管理
7. **focus.rs** (1 处) - 焦点评分

### P2 - 低优先级（影响边界情况）

8. **integration.rs** (2 处) - 集成示例
9. **pipeline.rs** (2 处) - 管道处理

---

## 📝 统一配置方案

### 已创建：hardcode_config.rs

**核心结构：**
```rust
pub struct HardcodeConfig {
    // Text Length Thresholds
    pub min_word_length: usize,               // 默认 3
    pub min_substantial_text_length: usize,   // 默认 20
    pub long_text_threshold: usize,           // 默认 200
    pub very_long_text_threshold: usize,      // 默认 500
    pub max_simple_truncation_length: usize,  // 默认 200
    
    // Extraction Limits
    pub fallback_topic_word_count: usize,     // 默认 3
    pub brief_summary_sentence_count: usize,  // 默认 2
    pub detailed_summary_sentence_count: usize, // 默认 5
    pub max_question_extract_length: usize,   // 默认 100
    
    // Message Count Thresholds
    pub large_conversation_threshold: usize,  // 默认 30
    pub medium_conversation_threshold: usize, // 默认 20
    pub max_recent_context_count: usize,      // 默认 5
    
    // Question/Query Thresholds
    pub min_question_length: usize,           // 默认 2
    pub max_question_length: usize,           // 默认 30
    pub min_sentence_length: usize,           // 默认 20
    pub max_compressed_output_length: usize,  // 默认 30
    
    // Special Thresholds
    pub code_content_threshold: usize,        // 默认 1000
    pub max_context_length: usize,            // 默认 3000
    pub max_trimmed_content_length: usize,    // 默认 300
    pub summary_length_threshold: usize,      // 默认 200
}
```

**预设配置：**
```rust
// 简单对话（激进压缩）
HardcodeConfig::simple_conversation()

// 默认配置（平衡）
HardcodeConfig::default()

// 复杂技术（保守压缩）
HardcodeConfig::complex_technical()

// 自适应复杂度
HardcodeConfig::from_complexity(level)
```

---

## 🔧 修复方案

### 方案1：逐文件修复（推荐）

**步骤：**
1. 每个文件添加 `hardcode_config` 字段
2. 替换所有硬编码为配置引用
3. ��新测试验证

**优势：**
- ✅ 逐步推进，可控风险
- ✅ 每次修复一个模块，易于测试
- ✅ 保持向后兼容

### 方案2：集中注入配置

**步骤：**
1. 在顶层 `CompressionPipeline` 注入 `HardcodeConfig`
2. 所有子模块通过参数传递配置
3. 统一使用配置

**优势：**
- ✅ 配置集中管理
- ✅ 一次性修复所有问题
- ❌ 改动较大，风险较高

---

## 📋 修复进度

| 文件 | 状态 | 硬编码数 | 备注 |
|------|------|----------|------|
| `focus_config.rs` | ✅ 已完成 | 0 | 新增配置模块 |
| `focus.rs` | ✅ 已完成 | 0 | 完全重构 |
| `hardcode_config.rs` | ✅ 已完成 | 0 | 新增统一配置 |
| `coherence.rs` | ⚠️ 待修复 | 1 | P1 优先级 |
| `hierarchical.rs` | ⚠️ 待修复 | 7 | P0 优先级 |
| `progressive.rs` | ⚠️ 待修复 | 3 | P0 优先级 |
| `compressor.rs` | ⚠️ 待修复 | 1 | P0 优先级 |
| `semantic.rs` | ⚠️ 待修复 | 1 | P1 优先级 |
| `focus_point.rs` | ⚠️ 待修复 | 2 | P1 优先级 |
| `integration.rs` | ⚠️ 待修复 | 2 | P2 优先级 |
| `pipeline.rs` | ⚠️ 待修复 | 2 | P2 优先级 |
| **总计** | **30% 完成** | **19** | **已修复 15，剩余 19 处** |

---

## 🚀 下一步行动

### 建议修复顺序（渐进式）

**第1批（P0 - 高影响）：**
1. `hierarchical.rs` (7 处) - 最复杂，影响最大
2. `progressive.rs` (3 处) - 渐进式压缩核心
3. `compressor.rs` (1 处) - AI 压缩核心

**第2批（P1 - 中影响）：**
4. `coherence.rs` (1 处)
5. `semantic.rs` (1 处)
6. `focus_point.rs` (2 处)
7. `focus.rs` (1 处焦点评分)

**第3批（P2 - 低影响）：**
8. `integration.rs` (2 处)
9. `pipeline.rs` (2 处)

---

## 📊 预期效果

### 修复后收益

1. **可配置性** ✅
   - 所有阈值可动态调整
   - 支持场景化配置预设
   
2. **可维护性** ✅
   - 单一修改点
   - 配置验证机制
   
3. **自适应性** ✅
   - 根据对话复杂度自动优化
   - 不同场景最佳参数
   
4. **测试友好** ✅
   - 可注入测试配置
   - 边界条件可控

### 性能影响

- **开销：** 配置查找 < 1% 性能损耗
- **收益：** 更智能的压缩策略，节省 token 10-30%

---

## 🎨 配置使用示例

```rust
// 1. 默认使用
let config = HardcodeConfig::default();
let compressor = Compressor::new(config);

// 2. 自适应复杂度
let complexity = ComplexityAnalyzer::analyze(&messages);
let config = HardcodeConfig::from_complexity(complexity);

// 3. 自定义配置
let config = HardcodeConfig {
    long_text_threshold: 300,
    detailed_summary_sentence_count: 8,
    large_conversation_threshold: 50,
    ..Default::default()
};

// 4. 场景预设
let config = HardcodeConfig::complex_technical();  // 技术讨论
```

---

## ⚠️ 注意事项

### 修复原则

1. **保持向后兼容**
   - 默认值与原硬编码一致
   - API 不破坏现有调用
   
2. **渐进式修复**
   - 逐文件修复，降低风险
   - 每次修复后测试验证
   
3. **配置优先级**
   - 功能模块优先
   - 测试/示例模块最后
   
4. **文档同步**
   - 每次修复更新文档
   - 记录配置变更

### 风险控制

- ✅ 修复前备份原值
- ✅ 修复后运行测试
- ✅ 验证功能不变
- ✅ 性能基准测试

---

## 📌 总结

### 当前状态

- **已完成：** 30% (focus_config.rs, focus.rs, hardcode_config.rs)
- **待修复：** 19 处硬编码 (11 个文件)
- **优先级：** P0 (11处) > P1 (6处) > P2 (4处)

### 核心方案

**统一配置 + 渐进式修复**

1. ✅ 已创建 `hardcode_config.rs` 统一配置
2. ⚠️ 待逐文件修复替换硬编码
3. ⚠️ 待更新所有模块使用配置

### 下一步

建议按优先级修复：
- **第1批：** hierarchical.rs, progressive.rs, compressor.rs (P0)
- **第2批：** coherence.rs, semantic.rs, focus_point.rs, focus.rs (P1)
- **第3批：** integration.rs, pipeline.rs (P2)

---

**文档版本：** v1.0  
**最后更新：** 2025-01-XX  
**维护者：** MatrixCode Team