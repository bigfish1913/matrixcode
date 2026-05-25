//! Tool Registry
//!
//! 工具注册中心 - 仅管理代理工具

use super::{Tool, ToolDefinition};
use super::proxy::{ProxyTool, ProxyMetadata};

/// 工具注册中心 - 仅管理代理工具（内置工具由 Agent 直接管理）
pub struct ToolRegistry {
    /// 代理工具（外部注入的工具）
    proxy_tools: Vec<ProxyTool>,
}

impl ToolRegistry {
    /// 创建空的工具注册中心
    pub fn new() -> Self {
        Self {
            proxy_tools: Vec::new(),
        }
    }
    
    /// 注册代理工具
    pub fn register(&mut self, tool: ProxyTool) {
        // 检查工具名称冲突
        if self.proxy_tools.iter().any(|t| t.definition().name == tool.definition().name) {
            log::warn!(
                "Proxy tool '{}' already exists, will be replaced",
                tool.definition().name
            );
            // 移除旧的
            self.proxy_tools.retain(|t| t.definition().name != tool.definition().name);
        }
        
        self.proxy_tools.push(tool);
    }
    
    /// 批量注册代理工具
    pub fn register_batch(&mut self, tools: Vec<ProxyTool>) {
        for tool in tools {
            self.register(tool);
        }
    }
    
    /// 获取所有代理工具定义（用于提交给大模型）
    pub fn proxy_definitions(&self) -> Vec<ToolDefinition> {
        self.proxy_tools.iter().map(|t| t.definition()).collect()
    }
    
    /// 获取所有代理工具
    pub fn proxy_tools(&self) -> &[ProxyTool] {
        &self.proxy_tools
    }
    
    /// 查找代理工具
    pub fn find_proxy(&self, name: &str) -> Option<&ProxyTool> {
        self.proxy_tools.iter().find(|t| t.definition().name == name)
    }
    
    /// 获取代理工具数量
    pub fn count(&self) -> usize {
        self.proxy_tools.len()
    }
    
    /// 检查是否是代理工具
    pub fn is_proxy(&self, name: &str) -> bool {
        self.proxy_tools.iter().any(|t| t.definition().name == name)
    }
    
    /// 获取代理工具元数据
    pub fn get_metadata(&self, name: &str) -> Option<&ProxyMetadata> {
        self.proxy_tools
            .iter()
            .find(|t| t.definition().name == name)
            .map(|t| t.metadata())
    }
    
    /// 清空所有代理工具
    pub fn clear(&mut self) {
        self.proxy_tools.clear();
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_registry_creation() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.count(), 0);
    }
    
    #[test]
    fn test_register_proxy_tool() {
        let mut registry = ToolRegistry::new();
        
        let proxy_tool = ProxyTool::new(
            ToolDefinition {
                name: "custom_search".to_string(),
                description: "自定义搜索".to_string(),
                parameters: json!({"type": "object"}),
            },
            ProxyMetadata {
                tool_type: "search".to_string(),
                endpoint: None,
                timeout_ms: 1000,
                custom: None,
            },
        );
        
        registry.register(proxy_tool);
        
        assert_eq!(registry.count(), 1);
        assert!(registry.is_proxy("custom_search"));
        assert!(registry.find_proxy("custom_search").is_some());
    }
    
    #[test]
    fn test_proxy_definitions() {
        let mut registry = ToolRegistry::new();
        
        registry.register(ProxyTool::new(
            ToolDefinition {
                name: "test".to_string(),
                description: "Test".to_string(),
                parameters: json!({}),
            },
            ProxyMetadata {
                tool_type: "test".to_string(),
                endpoint: None,
                timeout_ms: 1000,
                custom: None,
            },
        ));
        
        let definitions = registry.proxy_definitions();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "test");
    }
}