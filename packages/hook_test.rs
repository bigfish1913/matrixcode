//! MatrixCode Hooks 测试示例
//! 
//! 演示如何使用 ToolHook 系统来拦截和修改工具执行

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

// 模拟 hook 系统核心 trait
#[async_trait]
pub trait ToolHook: Send + Sync {
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool;
    fn applies_to(&self) -> Vec<&str> { Vec::new() }
    
    fn applies_to_tool(&self, tool_name: &str) -> bool {
        let applies_to = self.applies_to();
        applies_to.is_empty() || applies_to.iter().any(|t| *t == tool_name)
    }
    
    async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult>;
    async fn post_execute(&self, tool_name: &str, params: &Value, result: &str) -> Result<String>;
}

#[derive(Debug, Clone)]
pub enum HookResult {
    Continue,
    Block { reason: String, details: Option<String> },
    Modify(Value),
}

// === 示例 Hook 1: 日志 Hook ===
pub struct LoggingHook {
    enabled: bool,
}

impl LoggingHook {
    pub fn new() -> Self {
        Self { enabled: true }
    }
}

#[async_trait]
impl ToolHook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult> {
        println!("🔍 [LOG] 准备执行工具: {}", tool_name);
        println!("📋 [LOG] 参数: {}", serde_json::to_string_pretty(params).unwrap_or_default());
        Ok(HookResult::Continue)
    }
    
    async fn post_execute(&self, tool_name: &str, _params: &Value, result: &str) -> Result<String> {
        println!("✅ [LOG] 工具执行完成: {}", tool_name);
        println!("📊 [LOG] 结果长度: {} 字节", result.len());
        Ok(result.to_string())
    }
}

// === 示例 Hook 2: 安全检查 Hook ===
pub struct SecurityHook {
    blocked_paths: Vec<String>,
}

impl SecurityHook {
    pub fn new() -> Self {
        Self {
            blocked_paths: vec![
                "/etc/passwd".to_string(),
                "/etc/shadow".to_string(),
                ".env".to_string(),
                "credentials.json".to_string(),
            ],
        }
    }
}

#[async_trait]
impl ToolHook for SecurityHook {
    fn name(&self) -> &str {
        "security"
    }
    
    fn is_enabled(&self) -> bool {
        true
    }
    
    fn applies_to(&self) -> Vec<&str> {
        vec!["write", "edit", "read"]
    }
    
    async fn pre_execute(&self, _tool_name: &str, params: &Value) -> Result<HookResult> {
        if let Some(path) = params["path"].as_str() {
            for blocked in &self.blocked_paths {
                if path.contains(blocked) {
                    return Ok(HookResult::Block {
                        reason: format!("🚫 安全拦截: 禁止访问敏感文件 '{}'", blocked),
                        details: Some("此路径被安全策略保护。如需访问，请联系管理员。".to_string()),
                    });
                }
            }
        }
        Ok(HookResult::Continue)
    }
    
    async fn post_execute(&self, _tool_name: &str, _params: &Value, result: &str) -> Result<String> {
        Ok(result.to_string())
    }
}

// === 示例 Hook 3: 参数修改 Hook ===
pub struct AutoFormatHook;

#[async_trait]
impl ToolHook for AutoFormatHook {
    fn name(&self) -> &str {
        "auto_format"
    }
    
    fn is_enabled(&self) -> bool {
        true
    }
    
    fn applies_to(&self) -> Vec<&str> {
        vec!["write"]
    }
    
    async fn pre_execute(&self, _tool_name: &str, params: &Value) -> Result<HookResult> {
        if let Some(content) = params["content"].as_str() {
            // 自动格式化 JSON 内容
            if let Ok(json_value) = serde_json::from_str::<Value>(content) {
                let formatted = serde_json::to_string_pretty(&json_value)?;
                let mut new_params = params.clone();
                new_params["content"] = Value::String(formatted);
                println!("✨ [FORMAT] 已自动格式化 JSON 内容");
                return Ok(HookResult::Modify(new_params));
            }
        }
        Ok(HookResult::Continue)
    }
    
    async fn post_execute(&self, _tool_name: &str, _params: &Value, result: &str) -> Result<String> {
        Ok(result.to_string())
    }
}

// === Hook 注册中心 ===
pub struct HookRegistry {
    hooks: Vec<Box<dyn ToolHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }
    
    pub fn register(&mut self, hook: Box<dyn ToolHook>) {
        self.hooks.push(hook);
    }
    
    pub async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult> {
        let mut current_params = params.clone();
        
        for hook in &self.hooks {
            if !hook.is_enabled() || !hook.applies_to_tool(tool_name) {
                continue;
            }
            
            let result = hook.pre_execute(tool_name, &current_params).await?;
            
            match result {
                HookResult::Block { .. } => return Ok(result),
                HookResult::Modify(new_params) => current_params = new_params,
                HookResult::Continue => {},
            }
        }
        
        if current_params != *params {
            Ok(HookResult::Modify(current_params))
        } else {
            Ok(HookResult::Continue)
        }
    }
    
    pub async fn post_execute(&self, tool_name: &str, params: &Value, result: &str) -> Result<String> {
        let mut current_result = result.to_string();
        
        for hook in &self.hooks {
            if hook.is_enabled() && hook.applies_to_tool(tool_name) {
                current_result = hook.post_execute(tool_name, params, &current_result).await?;
            }
        }
        
        Ok(current_result)
    }
}

// === 测试用例 ===

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 MatrixCode Hooks 测试\n");
    println!("{}\n", "=".repeat(60));
    
    // 创建 Hook 注册中心
    let mut registry = HookRegistry::new();
    
    // 注册多个 hooks
    registry.register(Box::new(LoggingHook::new()));
    registry.register(Box::new(SecurityHook::new()));
    registry.register(Box::new(AutoFormatHook));
    
    println!("📦 已注册 Hooks:");
    for (i, hook) in registry.hooks.iter().enumerate() {
        println!("  {}. {} (适用于: {:?})", 
            i + 1, 
            hook.name(), 
            hook.applies_to()
        );
    }
    println!("{}\n", "=".repeat(60));
    
    // 测试场景 1: 正常执行
    println!("📝 测试场景 1: 正常写入文件\n");
    let params = serde_json::json!({
        "path": "test.txt",
        "content": "Hello, World!"
    });
    
    match registry.pre_execute("write", &params).await? {
        HookResult::Continue => println!("✅ Hook 允许执行\n"),
        HookResult::Block { reason, .. } => println!("❌ {}\n", reason),
        HookResult::Modify(new_params) => {
            println!("🔄 Hook 修改了参数:");
            println!("{}\n", serde_json::to_string_pretty(&new_params)?);
        }
    }
    
    println!("{}\n", "=".repeat(60));
    
    // 测试场景 2: 安全拦截
    println!("🔒 测试场景 2: 尝试访问敏感文件\n");
    let params = serde_json::json!({
        "path": ".env",
        "content": "SECRET=password123"
    });
    
    match registry.pre_execute("write", &params).await? {
        HookResult::Continue => println!("✅ Hook 允许执行\n"),
        HookResult::Block { reason, details } => {
            println!("❌ {}", reason);
            if let Some(detail) = details {
                println!("📝 详情: {}", detail);
            }
            println!();
        }
        HookResult::Modify(_) => println!("🔄 Hook 修改了参数\n"),
    }
    
    println!("{}\n", "=".repeat(60));
    
    // 测试场景 3: 自动格式化
    println!("✨ 测试场景 3: 自动格式化 JSON\n");
    let params = serde_json::json!({
        "path": "config.json",
        "content": "{\"name\":\"test\",\"version\":\"1.0\",\"enabled\":true}"
    });
    
    match registry.pre_execute("write", &params).await? {
        HookResult::Continue => println!("✅ Hook 允许执行\n"),
        HookResult::Block { reason, .. } => println!("❌ {}\n", reason),
        HookResult::Modify(new_params) => {
            println!("🔄 Hook 修改了参数:");
            println!("原内容: {}", params["content"].as_str().unwrap());
            println!("新内容: {}", new_params["content"].as_str().unwrap());
            println!();
        }
    }
    
    println!("{}\n", "=".repeat(60));
    
    // 测试场景 4: Post-execution Hook
    println!("📊 测试场景 4: Post-execution Hook\n");
    let params = serde_json::json!({
        "path": "test.txt",
        "content": "Test content"
    });
    
    let original_result = "文件写入成功";
    let final_result = registry.post_execute("write", &params, original_result).await?;
    
    println!("原始结果: {}", original_result);
    println!("最终结果: {}", final_result);
    println!();
    
    println!("{}\n", "=".repeat(60));
    println!("✅ 所有测试完成！");
    
    Ok(())
}