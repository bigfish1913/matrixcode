# Session Compression Fix Verification

## 修复内容总结

### 1. Agent 结构修改
在 `packages/core/src/agent/types.rs` 中：
- 添加 `full_messages` 字段：保存完整消息历史（用于显示和 session 存储）
- `messages` 字段：保存压缩后的消息（用于 API 调用）

### 2. AgentBuilder 修改
在 `packages/core/src/agent/builder.rs` 中：
- 添加 `initial_messages` 字段：支持从 session 恢复时直接设置消息
- 添加 `.initial_messages()` 方法：Builder 模式设置初始消息

### 3. Agent 运行时修改
在 `packages/core/src/agent/run.rs` 中：
- `Agent::new()`：初始化 `messages` 和 `full_messages` 都为 `initial_messages`
- 压缩逻辑：在压缩前保存 `full_messages`，压缩只修改 `messages`
- `set_messages()`：同时设置 `messages` 和 `full_messages`
- `get_full_messages()`：获取完整消息（用于显示和存储）
- `get_messages()`：获取压缩消息（用于 API）

### 4. Session 保存修改
在 `packages/cli/src/terminal/session.rs` 中：
- `save_after_turn()`：正确区分 `full_messages` 和 `compressed_messages`
- 添加压缩历史记录：记录每次压缩的详细信息

### 5. Agent 创建修改
在 `packages/cli/src/terminal/agent.rs` 中：
- 使用 `.initial_messages()` 直接设置恢复的消息，而不是创建后再调用 `set_messages()`

## 修复效果预期

### Token 数量改善
- **修复前**：`compressed_messages` 与 `full_messages` 相同（194 条），106,647 tokens
- **修复后**：`compressed_messages` 应显著小于 `full_messages`，token 数量减少 60%+

### 存储效率改善
- **修复前**：438KB（两份相同数据）
- **修复后**：预计减少到 ~200KB（只存储一份完整 + 一份压缩）

### API 成本改善
- **修复前**：每次 API 调用发送 106K+ tokens
- **修复后**：每次 API 调用发送压缩后的消息（约 40K tokens）

### 响应速度改善
- 更小的上下文 = 更快的 API 响应
- 更快的 session 加载（文件更小）

## 验证步骤

1. 启动新的 session：`matrixcode`
2. 进行一些对话和工具调用
3. 触发压缩：当 token 数量超过 40% 上下文窗口时
4. 保存 session：使用 `/save` 命令
5. 检查 session 文件：
   ```bash
   cat ~/.matrix/sessions/<session-id>.json | jq '{
     full_count: (.full_messages | length),
     compressed_count: (.compressed_messages | length),
     compression_history: .metadata.compression_history
   }'
   ```
6. 验证：
   - `compressed_count` 应小于 `full_count`
   - `compression_history` 应有记录

## 测试命令

### 快速验证（5分钟）
```bash
# 1. 启动 matrixcode
matrixcode

# 2. 进行一些对话（触发工具调用）
> hi
> read packages/core/src/agent/types.rs
> read packages/core/src/compress/config.rs
> grep compress packages/core/src/agent/run.rs

# 3. 检查压缩统计
# 在 TUI 中查看 debug panel，应该看到压缩信息

# 4. 保存 session
/save test_session

# 5. 检查 session 文件
ls -lh ~/.matrix/sessions/*.json | grep test
cat ~/.matrix/sessions/<test-session-id>.json | jq '{
  full: (.full_messages | length),
  compressed: (.compressed_messages | length),
  history: .metadata.compression_history
}'
```

### 详细验证（10分钟）
```bash
# 1. 恢复之前的 session
matrixcode --resume <session-id>

# 2. 检查消息数量
# 在 TUI 的 debug panel 应显示：
# - Full messages: 194
# - Compressed messages: 应该更少（取决于压缩策略）

# 3. 进行更多对话，触发压缩
# 当 context usage 超过 40% 时，应该看到压缩消息

# 4. 再次保存并检查
/save
cat ~/.matrix/sessions/<session-id>.json | jq '.metadata.compression_history'
```

## 成功标准

✅ **基础成功**：
- 编译无错误
- Session 文件中 `compressed_messages` ≠ `full_messages`（长度不同）
- `compression_history` 有记录

✅ **完整成功**：
- Token 数量减少 50%+
- 文件大小减少 40%+
- API 响应速度提升

## 问题诊断

### 如果压缩未生效
检查：
1. `compression_config.threshold` 是否正确（默认 0.4）
2. `estimate_total_tokens` 是否正确估算
3. 压缩日志是否出现在 debug panel

### 如果 session 仍然很大
检查：
1. `save_after_turn` 是否正确调用
2. `compression_history` 是否有记录
3. 工具结果是否被截断（检查 `tool_result` 内容长度）

### 如果压缩消息等于完整消息
检查：
1. Agent 的 `full_messages` 字段是否正确初始化
2. 压缩逻辑是否执行（查看日志）
3. `set_messages` 是否同时设置两个字段