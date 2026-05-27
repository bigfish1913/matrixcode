# 设计方案: MatrixCode 上下文压缩机制优化

日期: 2025-05-23

## 核心目标

- 智能评分系统：引入AI辅助评分，可选轻量/深度模式，默认轻量
- 工具结果压缩：超过阈值的内容自动生成智能摘要，保留关键信息
- 消息依赖追踪：ToolUse ↔ ToolResult 成对保留，避免对话断裂
- 动态权重调整：根据对话阶段自动调整评分权重

## 架构设计

```
packages/core/src/compress/
├── mod.rs              # 模块入口
├── config.rs           # 配置（新增阶段权重配置）
├── types.rs            # 类型定义（新增依赖图、压缩模式）
├── compressor.rs       # 原有评分逻辑（重构）
├── pipeline.rs         # 【新增】压缩管道协调器
├── phase_detector.rs   # 【新增】对话阶段检测
├── dependency.rs       # 【新增】消息依赖追踪
├── scorer.rs           # 【新增】智能评分器
├── summarizer.rs       # 【新增】内容摘要生成
├── tool_compressor.rs  # 【新增】工具结果压缩
```

### Pipeline 流程

```
CompressionPipeline.execute(messages)
    │
    ├─ Phase 1: 预处理
    │   ├─ PhaseDetector.detect() → InitialRequest/ActiveDev/Finalizing
    │   ├─ DependencyGraph.build() → ToolUse↔ToolResult配对
    │   └─ 标记关键消息
    │
    ├─ Phase 2: 智能评分
    │   ├─ Scorer.score_all(messages, phase_weights)
    │   │   ├─ 规则评分 (基础逻辑)
    │   │   ├─ AI辅助评分 (可选，ai_mode=light/deep)
    │   │   └─ 依赖链加分 (成对+50分)
    │   └─ 返回评分列表
    │
    ├─ Phase 3: 内容压缩
    │   ├─ ToolCompressor.compress_large_results()
    │   │   ├─ 内容<500 tokens → 保留原样
    │   │   ├─ 内容<2000 tokens → 轻量摘要
    │   │   └─ 内容>=2000 tokens → 深度摘要(ai_mode=deep)
    │   └─ 替换原消息中的ContentBlock
    │
    ├─ Phase 4: 选择保留
    │   ├─ 按评分排序
    │   ├─ 保证依赖链完整性（成对必须同时保留）
    │   ├─ 按target_ratio选择保留数量
    │   └─ 返回压缩后消息列表
```

## 数据模型 / 核心实体

```rust
/// 对话阶段
pub enum ConversationPhase {
    InitialRequest,     // 用户刚提出请求
    ActiveDevelopment,  // 正在执行工具操作
    Finalizing,         // 任务即将完成
}

/// 压缩模式
pub enum AiCompressionMode {
    None,               // 纯规则评分
    Light,              // 轻量AI辅助（fast_model）
    Deep,               // 深度AI分析
}

/// 消息依赖关系
pub struct MessageDependency {
    pub tool_use_idx: usize,
    pub tool_result_idx: usize,
    pub tool_name: String,
    pub is_critical: bool,
}

/// 依赖图
pub struct DependencyGraph {
    pub dependencies: Vec<MessageDependency>,
    pub message_to_deps: HashMap<usize, Vec<usize>>,
}

/// 阶段权重配置
pub struct PhaseWeights {
    pub first_msg_bonus: f64,
    pub user_msg_bonus: f64,
    pub tool_use_bonus: f64,
    pub tool_result_bonus: f64,
    pub critical_tool_bonus: f64,
    pub dependency_pair_bonus: f64,
}

/// 评分结果
pub struct ScoredMessage {
    pub index: usize,
    pub message: Message,
    pub base_score: f64,
    pub ai_score: Option<f64>,
    pub dependency_bonus: f64,
    pub final_score: f64,
    pub compressed_content: Option<MessageContent>,
}
```

## 关键接口 / API

```rust
// pipeline.rs
pub struct CompressionPipeline;

impl CompressionPipeline {
    pub async fn execute(
        &self,
        messages: &[Message],
        ai_mode: AiCompressionMode,
    ) -> Result<Vec<Message>>;
}

// phase_detector.rs
pub struct PhaseDetector;
impl PhaseDetector {
    pub fn detect(messages: &[Message]) -> ConversationPhase;
}

// dependency.rs
pub struct DependencyBuilder;
impl DependencyBuilder {
    pub fn build(messages: &[Message]) -> DependencyGraph;
}

// scorer.rs
pub struct Scorer;
impl Scorer {
    pub async fn score_all(
        &self,
        messages: &[Message],
        weights: &PhaseWeights,
        deps: &DependencyGraph,
        ai_mode: AiCompressionMode,
    ) -> Result<Vec<ScoredMessage>>;
}

// summarizer.rs
pub struct Summarizer;
impl Summarizer {
    pub async fn summarize_light(&self, content: &str) -> Result<String>;
    pub async fn summarize_deep(&self, content: &str) -> Result<String>;
}

// tool_compressor.rs
pub struct ToolCompressor;
impl ToolCompressor {
    pub async fn compress_results(
        &self,
        messages: &[Message],
        ai_mode: AiCompressionMode,
    ) -> Result<Vec<Message>>;
}
```

## 技术方案

- **方案选择**: 方案 A - 分层智能压缩架构
- **理由**: 
  - 分层处理职责清晰
  - AI辅助可选，成本可控
  - 依赖链保证对话连贯
  - 动态权重适应不同场景

## 错误处理策略

| 场景 | 处理策略 | 影响 |
|------|----------|------|
| AI调用超时 | 降级为规则评分 | 功能可用，智能性降低 |
| AI调用失败 | 降级为规则评分 | 功能可用 |
| 内容压缩失败 | 保留原内容 | token可能超限，但内容完整 |
| 依赖链断裂 | 强制恢复配对 | 保证对话连贯 |
| 阶段检测错误 | 使用默认权重 | 评分可能不精准 |

```rust
impl CompressionPipeline {
    fn ensure_dependency_integrity(
        messages: Vec<Message>,
        deps: &DependencyGraph,
    ) -> Vec<Message> {
        // 检查所有配对是否完整，强制修复断裂
    }
}
```

## 测试策略

### 测试文件结构
```
packages/core/src/compress/tests/
├── test_phase_detector.rs
├── test_dependency.rs
├── test_scorer.rs
├── test_summarizer.rs
├── test_pipeline.rs
└── test_fallback.rs
```

### 测试覆盖率目标

| 模块 | 覆盖率目标 | 重点 |
|------|------------|------|
| PhaseDetector | 90% | 阶段检测准确性 |
| DependencyBuilder | 95% | 配对完整性 |
| Scorer | 85% | 评分逻辑 |
| Summarizer | 80% | 摘要质量 |
| Pipeline | 90% | 集成流程 |
| Fallback | 95% | 降级策略 |

## 约束与风险

### 约束
- AI辅助使用fast_model（如claude-3-5-haiku），成本≈主模型1/10
- 保持原有API向后兼容，新增可选参数
- 依赖链完整性必须保证

### 风险
- AI调用可能增加延迟 → 异步执行，超时降级
- 摘要质量依赖模型能力 → 提供截断fallback
- 评分规则可能需要迭代 → 配置化权重

## 验收标准

- ✅ 压缩后对话连贯性保持
- ✅ 大工具结果（>500 tokens）被智能摘要
- ✅ 不同阶段权重动态调整
- ✅ AI辅助可选且成本可控
- ✅ 测试覆盖率达标（>80%）
- ✅ 原有API向后兼容