# 搜索优化改进说明

## 背景
用户反馈：AI 在使用 glob、search、bash 等工具搜索文件时，有时会进行全目录搜索，导致效率低下。

## 优化内容

### 1. Prompt 级别指导（core/src/prompt/constants.rs）

#### 在工具选择决策链中新增"第2步：限制搜索范围"

**修改位置：**
- `SYSTEM_PROMPT_TOOL_DECISION_GENERIC` (第 238-266 行)
- `SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH` (第 268-297 行)

**新增内容：**
```
第2步：限制搜索范围（重要！）
避免全目录搜索，优先精准定位：
- 先探索结构：ls() 查看顶层目录 → ls({ path: "src" }) 查看子目录
- 使用路径参数：grep({ path: "src/api" }) 而非 grep()
- 使用过滤参数：grep({ type: "rs" }) 或 glob({ pattern: "*.rs" })
- 分阶段搜索：先小范围，再逐步扩大
```

**效果：**
- AI 在搜索前会先思考是否需要限制范围
- 强调了"先探索结构，再精准搜索"的最佳实践
- 将"限制搜索范围"提升为决策链的必经步骤

### 2. 工具参数描述强化

#### grep 工具（core/src/tools/grep.rs）

**修改内容：**
- `path` 参数：添加 `⚠️ 尽量指定路径避免全目录搜索，如 'src/api' 而非 '.'`
- `glob` 参数：添加 `推荐使用以缩小搜索范围`
- `type` 参数：添加 `推荐使用以提升搜索速度`

#### search 工具（core/src/tools/search.rs）

**修改内容：**
- `path` 参数：添加 `⚠️ 尽量指定路径避免全目录搜索，如 'src' 而非 '.'`
- `glob` 参数：添加 `推荐使用以缩小搜索范围`

#### glob 工具（core/src/tools/glob.rs）

**修改内容：**
- `path` 参数：添加 `⚠️ 尽量指定路径避免全目录搜索，如 'src' 而非 '.'`

## 最佳实践示例

### ❌ 不推荐（全目录搜索）
```javascript
grep({ pattern: "handle_request" })
glob({ pattern: "**/*.rs" })
search({ pattern: "TODO" })
```

### ✅ 推荐（限制范围）
```javascript
// 先探索结构
ls()  // 查看顶层目录
ls({ path: "src" })  // 定位到子目录

// 再精准搜索
grep({ pattern: "handle_request", path: "src/api", type: "rs" })
glob({ pattern: "*.rs", path: "src/models" })
search({ pattern: "TODO", path: "docs" })
```

## 预期效果

1. **减少不必要的全目录扫描**：AI 会优先使用路径参数
2. **提升搜索效率**：通过文件类型过滤（type/glob）减少扫描文件数
3. **更好的用户体验**：搜索结果更精准，响应更快
4. **遵循"分阶段探索"模式**：先 ls 看结构，再精准搜索

## 技术细节

### Prompt 修改位置
- `core/src/prompt/constants.rs`
  - 第 238-266 行：`SYSTEM_PROMPT_TOOL_DECISION_GENERIC`
  - 第 268-297 行：`SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH`

### 工具描述修改位置
- `core/src/tools/grep.rs`：第 96-108 行
- `core/src/tools/search.rs`：第 47-53 行
- `core/src/tools/glob.rs`：第 51-54 行

### 测试验证
```bash
cd core && cargo test --lib tools::mod::tests
```
所有测试通过 ✓

## 未来改进方向

1. **监控搜索行为**：统计全目录搜索频率，评估优化效果
2. **智能路径建议**：根据项目结构自动推荐搜索路径
3. **搜索结果缓存**：对频繁搜索的模式建立缓存
4. **警告机制**：当检测到大规模搜索时提示用户

## 总结

本次优化通过三个层面改进搜索行为：
1. **决策层面**：在 Prompt 中明确"限制搜索范围"为必经步骤
2. **工具层面**：在参数描述中强化路径/过滤参数的使用
3. **实践层面**：提供清晰的"先探索，再搜索"最佳实践示例

这符合"最小改动完整解决问题"的原则，通过 Prompt 和工具描述的微调，引导 AI 形成更好的搜索习惯。