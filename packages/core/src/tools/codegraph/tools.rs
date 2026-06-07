//! Tool implementations for CodeGraph.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

use super::manager::CodeGraphManager;
use crate::approval::RiskLevel;
use crate::tools::{Tool, ToolDefinition};

/// Tool for searching symbols in CodeGraph index.
pub struct CodeGraphSearchTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphSearchTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_search".to_string(),
            description: "搜索代码符号（函数、类、方法、变量）。查找代码定义时必须优先使用此工具，比 grep 快 10-100 倍。返回符号位置、签名、文档。grep 仅用于搜索字符串内容（如错误消息）。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "符号名称搜索模式（支持模糊匹配）"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 20）",
                        "default": 20
                    }
                },
                "required": ["pattern"]
            }),
            is_priority: true,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing pattern parameter"))?;
        let limit = args["limit"].as_u64().unwrap_or(20) as usize;

        let nodes = self.manager.search(pattern, limit)?;

        Ok(serde_json::to_string(&json!({
            "nodes": nodes,
            "query": pattern,
            "total_count": nodes.len()
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for finding callers of a symbol.
pub struct CodeGraphCallersTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphCallersTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphCallersTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_callers".to_string(),
            description: "查找调用指定符号的所有函数/方法。分析调用关系时必须优先使用，比 grep 追溯更准确。grep 仅用于搜索字符串内容。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "符号 ID 或名称"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 10）",
                        "default": 10
                    }
                },
                "required": ["symbol"]
            }),
            is_priority: true,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let symbol = args["symbol"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing symbol parameter"))?;
        let limit = args["limit"].as_u64().unwrap_or(10) as usize;

        let nodes = self.manager.callers(symbol, limit)?;

        Ok(serde_json::to_string(&json!({
            "callers": nodes,
            "symbol": symbol,
            "total_count": nodes.len()
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for finding callees of a symbol.
pub struct CodeGraphCalleesTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphCalleesTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphCalleesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_callees".to_string(),
            description: "查找指定符号调用的所有函数/方法。分析执行流程时必须优先使用，比 grep 追踪更准确。grep 仅用于搜索字符串内容。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "符号 ID 或名称"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量限制（默认 10）",
                        "default": 10
                    }
                },
                "required": ["symbol"]
            }),
            is_priority: true,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let symbol = args["symbol"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing symbol parameter"))?;
        let limit = args["limit"].as_u64().unwrap_or(10) as usize;

        let nodes = self.manager.callees(symbol, limit)?;

        Ok(serde_json::to_string(&json!({
            "callees": nodes,
            "symbol": symbol,
            "total_count": nodes.len()
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for checking CodeGraph status.
pub struct CodeGraphStatusTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphStatusTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_status".to_string(),
            description: "检查 CodeGraph 索引状态。返回文件数、节点数、边数、支持的语言等信息。"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, _args: Value) -> Result<String> {
        let status = self.manager.status()?;
        Ok(serde_json::to_string(&status)?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for syncing CodeGraph index.
pub struct CodeGraphSyncTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphSyncTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphSyncTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_sync".to_string(),
            description: "手动同步 CodeGraph 索引。当代码库有变化但自动同步未触发时使用，确保搜索结果是最新的。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, _args: Value) -> Result<String> {
        self.manager.sync().await?;
        Ok(serde_json::to_string(
            &json!({"success": true, "message": "CodeGraph index synced"}),
        )?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Tool for querying indexed files.
pub struct CodeGraphFilesTool {
    manager: Arc<CodeGraphManager>,
}

impl CodeGraphFilesTool {
    pub fn new(project_path: &Path) -> Self {
        Self {
            manager: Arc::new(CodeGraphManager::new(project_path)),
        }
    }
}

#[async_trait]
impl Tool for CodeGraphFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_files".to_string(),
            description: "查询索引中的文件列表。返回文件路径、语言类型、符号数量。支持按语言过滤，比 glob 更智能，能显示每个文件的符号密度。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "按语言过滤（如 'rust', 'typescript', 'python'），不填则返回所有文件"
                    }
                }
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let language = args.get("language").and_then(|v| v.as_str());
        let files = self.manager.files(language)?;
        
        Ok(serde_json::to_string(&json!({
            "files": files,
            "total_count": files.len(),
            "filter": language
        }))?)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Create CodeGraph tools for a project path.
pub fn codegraph_tools(project_path: &Path) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CodeGraphSearchTool::new(project_path)),
        Box::new(CodeGraphCallersTool::new(project_path)),
        Box::new(CodeGraphCalleesTool::new(project_path)),
        Box::new(CodeGraphStatusTool::new(project_path)),
        Box::new(CodeGraphSyncTool::new(project_path)),
        Box::new(CodeGraphFilesTool::new(project_path)),
    ]
}

/// Create CodeGraph tools with automatic project root detection.
pub fn codegraph_tools_with_auto_detect(start_path: &Path) -> Vec<Box<dyn Tool>> {
    let project_path = super::project::find_project_root(start_path);
    codegraph_tools(&project_path)
}

/// Check if CodeGraph tools should be injected.
pub fn should_inject_codegraph_tools(start_path: &Path) -> bool {
    super::install::is_codegraph_installed()
        && CodeGraphManager::with_auto_detect(start_path).is_initialized()
}

/// Create CodeGraph tools if installed.
pub fn codegraph_tools_if_installed(start_path: &Path) -> Vec<Box<dyn Tool>> {
    if should_inject_codegraph_tools(start_path) {
        codegraph_tools_with_auto_detect(start_path)
    } else {
        vec![]
    }
}
