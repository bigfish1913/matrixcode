# AgentTaskContext Builder Pattern

## 重构总结

将 `run_agent_task` 函数的 18 个参数封装到 `AgentTaskContext` 结构体中，使用 Builder 模式进行构建和验证。

## 改进前后对比

### 改进前

```rust
// 函数签名：18 个参数
#[allow(clippy::too_many_arguments)]
async fn run_agent_task(
    cancel_token: CancellationToken,
    event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    api_key: String,
    model: String,
    // ... 还有 14 个参数
) { ... }

// 调用：传递 18 个独立参数
run_agent_task(
    agent_cancel,
    agent_event_tx,
    agent_api_key,
    agent_model,
    // ... 还有 14 个参数
).await;
```

### 改进后

```rust
// 函数签名：1 个结构体参数
async fn run_agent_task(ctx: AgentTaskContext) {
    // 解构上下文
    let AgentTaskContext {
        cancel_token, event_tx, api_key, model, ...
    } = ctx;
    ...
}

// 调用：使用 Builder 模式构建上下文
let ctx_result = AgentTaskContextBuilder::new()
    .cancel_token(agent_cancel)
    .event_tx(agent_event_tx)
    .api_key(agent_api_key)
    .model(agent_model)
    .base_url(agent_base_url)
    .think(agent_think)
    .max_tokens(agent_max_tokens)
    .restored_messages(agent_restored_messages)
    .project_path(agent_project_path)
    .approve_mode(agent_approve_mode)
    .provider_type(agent_provider)
    .fast_model(agent_fast_model)
    .extra_headers(agent_extra_headers)
    .config(agent_config)
    .skills(agent_skills)
    .shared_approve_mode(agent_shared_approve_mode)
    .session_mgr(session_mgr_state)
    .task_rx(task_rx)
    .ask_rx(ask_rx)
    .build();

match ctx_result {
    Ok(ctx) => run_agent_task(ctx).await,
    Err(e) => {
        log::error!("Failed to build AgentTaskContext: {}", e);
        eprintln!("Configuration error: {}", e);
    }
}
```

## 验证功能

Builder 会自动验证以下规则：

1. **必填字段检查**：
   - `cancel_token`, `event_tx`, `api_key`, `model`, `base_url`
   - `think`, `max_tokens`, `restored_messages`, `approve_mode`
   - `provider_type`, `config`, `skills`, `shared_approve_mode`
   - `task_rx`, `ask_rx`

2. **业务规则验证**：
   - `api_key` 不能为空字符串
   - `model` 不能为空字符串
   - `base_url` 不能为空字符串
   - `max_tokens` 必须大于 0

3. **错误处理**：
```rust
pub enum AgentTaskContextError {
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
```

## 代码质量改进

- ✅ **消除 Long Parameter List 代码异味**：从 18 个参数减少到 1 个
- ✅ **类型安全**：编译时检查所有字段
- ✅ **运行时验证**：业务规则验证防止无效配置
- ✅ **可读性提升**：链式调用清晰表达意图
- ✅ **可扩展性**：添加新字段只需修改 Builder
- ✅ **错误处理**：统一的错误类型和消息

## 未来改进建议

1. **添加 Default trait**：
```rust
impl Default for AgentTaskContext {
    fn default() -> Self {
        Self {
            think: true,
            max_tokens: DEFAULT_MAX_TOKENS,
            approve_mode: ApproveMode::Auto,
            // ... 其他默认值
        }
    }
}
```

2. **添加辅助方法**：
```rust
impl AgentTaskContextBuilder {
    /// 从配置文件构建上下文
    pub fn from_config(config: &Config) -> Self {
        Self::new()
            .model(config.model.clone())
            .base_url(config.base_url.clone())
            // ...
    }

    /// 验证并构建，panic on error
    pub fn build_unwrap(self) -> AgentTaskContext {
        self.build().expect("Invalid AgentTaskContext")
    }
}
```

3. **添加单元测试**：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_missing_required_field() {
        let result = AgentTaskContextBuilder::new()
            .api_key("test".to_string())
            .build();
        assert!(matches!(result, Err(AgentTaskContextError::MissingField(_))));
    }

    #[test]
    fn test_builder_invalid_api_key() {
        let result = AgentTaskContextBuilder::new()
            .api_key("".to_string())
            // ... 其他必填字段
            .build();
        assert!(matches!(result, Err(AgentTaskContextError::InvalidConfig(_))));
    }
}
```

## 编译状态

- ✅ **编译通过**：无错误
- ✅ **无警告**：无 clippy 警告
- ✅ **类型检查**：所有类型正确