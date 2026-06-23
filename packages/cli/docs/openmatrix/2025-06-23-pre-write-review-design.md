# 设计方案: 写入前自动代码审核系统

日期: 2025-06-23

## 核心目标

在 `write/edit/multi_edit` 工具执行前，自动调用 AI 审核，确保写入代码的质量和安全性，避免低质量代码进入项目。

- **审核方式**: AI 自动审核 + 自动决策 + 展示结果（无需用户交互）
- **触发时机**: 每次 write/edit 都自动审核（默认行为）
- **自动决策规则**: 
  - ✅ 评分 >= 60 且无 Critical 问题 → 自动写入，在工具结果中展示审核摘要
  - ❌ 评分 < 60 或有 Critical 问题 → 阻止写入，返回审核报告和修改建议

## 审核内容（5项全选）

### 1. 代码质量检查
- 命名规范（函数、变量、文件名）
- 代码结构（单一职责、函数长度、嵌套层级）
- 注释完整性（关键逻辑、边界条件）
- 重复代码检测

### 2. 影响范围分析
- 修改会影响哪些模块
- 是否有依赖需要同步修改
- 是否破坏现有行为（除非明确要求）

### 3. 安全问题检查
- SQL 注入风险
- XSS 漏洞
- 敏感信息泄露（密钥、Token、密码）
- 权限检查缺失
- 文件路径穿越

### 4. 性能问题检查
- 算法复杂度分析
- 内存使用评估
- 不必要的循环/重复计算
- 资源泄漏风险

### 5. 最佳实践检查
- 错误处理是否完善
- 日志记录是否合理
- 测试覆盖是否需要
- 代码风格一致性

## 架构设计

### 方案选择: 独立审核模块（推荐）

**理由**: 
- 解耦性更好，审核逻辑独立于 approval 机制
- 可以单独调用，也支持通过 skill 调用
- 审核结果可以持久化，方便追踪

### 核心组件

```
┌─────────────────────────────────────────────────────────────┐
│                     execute_tool 流程                        │
│                                                              │
│  1. 检测工具类型 (write/edit/multi_edit)                    │
│  2. 调用 pre_write_review() 审核函数                         │
│     ↓                                                        │
│  ┌─────────────────────────────────────┐                    │
│  │ PreWriteReview                      │                    │
│  │ - 读取现有文件（如存在）             │                    │
│  │ - 解析新代码内容                     │                    │
│  │ - 调用 AI 审核（5项检查）            │                    │
│  │ - 生成审核报告                       │                    │
│  └─────────────────────────────────────┘                    │
│     ↓                                                        │
│  3. AI 自动决策                                              │
│     ┌─────────────────────────────────────┐                  │
│     │ 审核结果: 评分 85, 无严重问题        │                  │
│     │ 🟡 警告: 2 个                        │                  │
│     │ 🟢 建议: 1 个                        │                  │
│     │                                      │                  │
│     │ ✅ 自动写入（符合质量标准）           │                  │
│     └─────────────────────────────────────┘                  │
│     ↓                                                        │
│  4. 写入文件 + 在工具结果中展示审核摘要                       │
│                                                              │
│  (如果评分 < 60 或有 Critical 问题)                          │
│     ↓                                                        │
│  4. 阻止写入 + 返回审核报告 + 修改建议                        │
└─────────────────────────────────────────────────────────────┘
```

### 数据模型

#### PreWriteReviewInput
```rust
pub struct PreWriteReviewInput {
    pub tool_name: String,          // "write", "edit", "multi_edit"
    pub file_path: PathBuf,         // 目标文件路径
    pub existing_content: Option<String>,  // 现有内容（如文件存在）
    pub new_content: String,        // 新代码内容
    pub edit_info: Option<EditInfo>, // edit/multi_edit 的具体修改信息
}

pub struct EditInfo {
    pub old_string: String,
    pub new_string: String,
}
```

#### PreWriteReviewResult
```rust
pub struct PreWriteReviewResult {
    pub overall_score: u8,          // 0-100 总体评分
    pub issues: Vec<ReviewIssue>,   // 发现的问题列表
    pub impact_analysis: ImpactAnalysis, // 影响范围分析
    pub suggestions: Vec<String>,   // 改进建议
}

pub struct ReviewIssue {
    pub level: IssueLevel,          // Critical, Warning, Suggestion
    pub category: IssueCategory,    // Quality, Security, Performance, Practice
    pub message: String,            // 问题描述
    pub location: Option<String>,   // 代码位置（行号/函数名）
    pub fix_example: Option<String>, // 修复示例（严重问题时提供）
}

pub struct ImpactAnalysis {
    pub affected_modules: Vec<String>, // 影响的模块列表
    pub dependencies: Vec<String>,     // 需要同步修改的依赖
    pub breaking_changes: bool,        // 是否破坏现有行为
}

pub enum IssueLevel {
    Critical,   // 🔴 严重问题，阻止写入
    Warning,    // 🟡 警告，不影响写入决策
    Suggestion, // 🟢 建议改进
}
```

## 关键接口

### 1. pre_write_review() 函数
```rust
impl Agent {
    /// Pre-write review: analyze code quality before writing
    async fn pre_write_review(&mut self, input: PreWriteReviewInput) -> Result<PreWriteReviewResult> {
        // 1. Extract code content for review
        let code_for_review = match input.tool_name {
            "write" => input.new_content.clone(),
            "edit" | "multi_edit" => {
                // For edits, show the change context
                format!(
                    "Existing:\n{}\n\nNew:\n{}",
                    input.existing_content.unwrap_or_default(),
                    input.new_content
                )
            },
            _ => input.new_content.clone(),
        };

        // 2. Build review prompt
        let review_prompt = self.build_review_prompt(&input, &code_for_review);

        // 3. Call AI provider for review
        let response = self.provider.generate(&review_prompt).await?;

        // 4. Parse review result
        let result = parse_review_result(&response)?;

        Ok(result)
    }

    /// Build review prompt with 5-check framework
    fn build_review_prompt(&self, input: &PreWriteReviewInput, code: &str) -> String {
        format!(
            r#"Review this code before writing to '{}':

{}

Review checklist:
1. Code Quality: naming, structure, comments, duplication
2. Security: SQL injection, XSS, sensitive data, permissions
3. Performance: complexity, memory, unnecessary loops
4. Impact: affected modules, dependencies, breaking changes
5. Best Practices: error handling, logging, test coverage

Output format (JSON):
{
  "overall_score": 85,
  "issues": [
    {"level": "Critical", "category": "Security", "message": "...", "location": "...", "fix_example": "..."}
  ],
  "impact_analysis": {
    "affected_modules": [...],
    "dependencies": [...],
    "breaking_changes": false
  },
  "suggestions": [...]
}

Focus on actionable feedback. For critical issues, provide fix examples."#,
            input.file_path.display(),
            code
        )
    }
}
```

### 2. execute_tool() 修改
```rust
pub(crate) async fn execute_tool(&mut self, name: &str, input: Value) -> Result<String> {
    // Pre-write review for mutating tools
    let review_result = if matches!(name, "write" | "edit" | "multi_edit") {
        let review_input = PreWriteReviewInput::from_tool_input(name, &input)?;
        
        // Auto review (AI analyzes code quality)
        self.pre_write_review(review_input).await?
    } else {
        None
    };
    
    // Auto decision based on score and critical issues
    if let Some(result) = &review_result {
        let should_write = result.overall_score >= 60 
            && !result.issues.iter().any(|i| i.level == Critical);
        
        if !should_write {
            // Block write and return review report
            let report = format_review_report(&result);
            return Err(anyhow::anyhow!(
                "Write blocked by pre-review: score {} < 60 or has critical issues.\n\n{}",
                result.overall_score,
                report
            ));
        }
    }
    
    // Execute tool
    let tool_result = /* 执行工具 */;
    
    // Append review summary to result if reviewed
    if let Some(result) = &review_result {
        Ok(format!(
            "{}\n\n审核摘要: 评分 {}, {} 个警告, {} 个建议",
            tool_result,
            result.overall_score,
            result.issues.iter().filter(|i| i.level == Warning).count(),
            result.issues.iter().filter(|i| i.level == Suggestion).count()
        ))
    } else {
        Ok(tool_result)
    }
}
```

### 3. 格式化审核结果
```rust
fn format_review_summary(result: &PreWriteReviewResult) -> String {
    let critical_count = result.issues.iter().filter(|i| i.level == Critical).count();
    let warning_count = result.issues.iter().filter(|i| i.level == Warning).count();
    let suggestion_count = result.issues.iter().filter(|i| i.level == Suggestion).count();
    
    format!(
        r#"┌─ 写入前审核结果 ─────────────────────────────────────────
│ 总体评分: {} 分
│ 🔴 严重: {} 个
│ 🟡 警告: {} 个  
│ 🟢 建议: {} 个
│
│ 影响范围: {}
│ 破坏性变更: {}
│
│ {} 个模块受影响，{} 个依赖需同步修改
└───────────────────────────────────────────────────────────"#,
        result.overall_score,
        critical_count,
        warning_count,
        suggestion_count,
        result.impact_analysis.affected_modules.join(", "),
        if result.impact_analysis.breaking_changes { "⚠️ 是" } else { "✅ 否" },
        result.impact_analysis.affected_modules.len(),
        result.impact_analysis.dependencies.len()
    )
}

fn format_review_report(result: &PreWriteReviewResult) -> String {
    let mut output = String::new();
    
    output.push_str(&format_review_summary(result));
    output.push_str("\n\n=== 问题详情 ===\n\n");
    
    for issue in &result.issues {
        output.push_str(&format!(
            "{} {} [{}]\n  {}\n",
            issue.level.icon(),
            issue.category,
            issue.location.as_deref().unwrap_or("N/A"),
            issue.message
        ));
        
        if let Some(fix) = &issue.fix_example {
            output.push_str(&format!("  修复示例:\n{}\n", fix));
        }
    }
    
    output.push_str("\n=== 改进建议 ===\n");
    for (i, suggestion) in result.suggestions.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, suggestion));
    }
    
    output
}
```

## 技术方案

### 实现步骤

**Phase 1: 基础审核框架**
1. 创建 `PreWriteReviewInput` 和 `PreWriteReviewResult` 数据结构
2. 实现 `pre_write_review()` 函数
3. 修改 `execute_tool()` 集成审核流程
4. 实现审核结果格式化函数

**Phase 2: AI 审核调用**
1. 设计审核 prompt（5-check framework）
2. 实现 AI 调用接口
3. 解析 AI 返回的 JSON 结果
4. 处理 AI 调用失败的情况

**Phase 3: 自动决策逻辑**
1. 实现评分计算和决策规则
2. 处理写入成功场景（附加审核摘要）
3. 处理写入阻止场景（返回审核报告）
4. 日志记录审核结果

**Phase 4: 智能化增强**
1. 根据文件类型调整审核强度（src/*.rs 严格，docs/*.md 宽松）
2. 学习历史审核结果，优化审核策略
3. 支持自定义审核规则（配置文件）
4. 审核结果持久化，支持查询历史

## 错误处理策略

### 自动决策规则
- **写入通过**: 评分 >= 60 且无 Critical 问题
- **写入阻止**: 评分 < 60 或有 Critical 问题
- **阻止后行为**: 返回审核报告 + 修改建议，AI 根据建议重新生成代码

### AI 调用失败
- **降级策略**: 审核失败时，自动写入（因为无法判断质量问题）
- **日志记录**: 记录审核失败事件，但不阻止正常工作流

### 审核超时
- **超时设置**: 审核请求 30 秒超时
- **超时处理**: 超时时自动写入（降级策略），日志记录超时事件

## 测试策略

### 单元测试
- `PreWriteReviewInput::from_tool_input()` 解析正确性
- `format_review_summary()` 格式化输出
- `parse_review_result()` JSON 解析
- 自动决策逻辑测试（评分阈值、Critical 检测）

### 集成测试
- write 工具触发审核流程
- edit 工具触发审核流程
- 自动决策通过场景
- 自动决策阻止场景
- 审核失败降级流程

### 边界测试
- 大文件审核（性能）
- 审核超时处理
- AI 返回非 JSON 格式处理
- 空文件/空内容审核

## 约束与风险

### 约束
- 审核增加约 1-3 秒延迟（AI 调用时间）
- 审核消耗额外 API tokens（每次审核约 500-1000 tokens）
- 审核准确性依赖于 AI 模型能力

### 风险及应对
| 风险 | 影响 | 应对措施 |
|-----|------|---------|
| AI 审核误报 | 阻止正常写入 | 降低阈值到 60，允许小问题 |
| AI 审核漏报 | 低质量代码进入项目 | 持续优化 prompt，增加检查项 |
| 性能开销 | 写入延迟增加 | 异步审核，支持配置关闭 |
| API 成本 | tokens 消耗增加 | 智能审核（按文件类型） |

## 验收标准

### 功能验收
- ✅ write/edit/multi_edit 工具触发审核流程
- ✅ 审核结果包含 5 项检查内容
- ✅ 自动决策：评分 >= 60 且无 Critical → 写入；否则阻止
- ✅ 写入成功时在工具结果中展示审核摘要
- ✅ 写入阻止时返回审核报告 + 修改建议
- ✅ 审核失败时自动降级（继续写入）

### 性能验收
- ✅ 审核延迟 < 3 秒（90% 请求）
- ✅ 审核超时自动降级（30 秒）
- ✅ 不影响 normal approval 流程（审核关闭时）

### 用户体验验收
- ✅ 审核结果清晰易懂（图标 + 评分）
- ✅ 无需用户交互（全自动流程）
- ✅ 工具结果中包含审核摘要供参考
- ✅ 阻止时提供清晰的修改建议

## 扩展性设计

### 未来功能
1. **自定义审核规则**: 用户可配置额外检查项
2. **审核历史查询**: 查看历史审核结果和决策
3. **审核统计报告**: 统计代码质量趋势
4. **团队审核规则**: 共享审核配置
5. **自动修复**: AI 直接修复简单问题

### 配置示例
```yaml
# .matrix/review.yaml
pre_write_review:
  enabled: true
  timeout: 30s
  score_threshold: 60  # 写入通过的最低评分
  checks:
    - quality
    - security
    - performance
    - impact
    - practice
  
  file_type_rules:
    src/**/*.rs: strict     # 严格审核
    docs/**/*.md: loose     # 宽松审核
    tests/**/*.rs: medium   # 中等审核
```

---

**下一步**: 调用 `/om:plan` 生成技术方案后执行