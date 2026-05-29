//! 自定义工具注入示例
//!
//! 演示如何创建和使用自定义代理工具
//!
//! 运行方式：
//! ```bash
//! cargo run --example custom_tool_injection
//! ```

use matrixcode_core::{
    event::{AgentEvent, EventData, EventType},
    tools::{ProxyMetadata, ProxyTool, ToolDefinition},
};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 模拟的内部搜索服务
async fn search_internal_knowledge(query: &str, limit: usize) -> Result<String, String> {
    // 模拟延迟
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 模拟搜索结果
    let results = vec![
        "MatrixCode 是一个 AI 编程助手",
        "支持自定义工具注入",
        "事件驱动架构",
        "Rust 和 TypeScript 混合开发",
        "支持多种 LLM 提供商",
    ];

    let filtered: Vec<_> = results
        .iter()
        .filter(|r| r.contains(query) || query.is_empty())
        .take(limit)
        .collect();

    Ok(json!({
        "query": query,
        "total": filtered.len(),
        "results": filtered
    })
    .to_string())
}

/// 模拟的数据库查询服务
async fn query_database(sql: &str) -> Result<String, String> {
    // 模拟延迟
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 模拟查询结果
    Ok(json!({
        "sql": sql,
        "rows": [
            {"id": 1, "name": "Alice", "role": "admin"},
            {"id": 2, "name": "Bob", "role": "user"},
        ],
        "rowCount": 2
    })
    .to_string())
}

/// 执行自定义工具
async fn execute_custom_tool(
    tool_name: &str,
    tool_input: serde_json::Value,
    metadata: &ProxyMetadata,
) -> Result<String, String> {
    println!("🔧 执行工具: {}", tool_name);
    println!(
        "📥 输入参数: {}",
        serde_json::to_string_pretty(&tool_input).unwrap()
    );
    println!("📋 元数据: {:?}\n", metadata);

    // 根据工具类型执行不同逻辑
    match tool_name {
        "search_knowledge" => {
            let query = tool_input
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'query' parameter")?;

            let limit = tool_input
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize;

            search_internal_knowledge(query, limit).await
        }

        "query_database" => {
            let sql = tool_input
                .get("sql")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'sql' parameter")?;

            // 安全检查：防止危险 SQL
            if sql.to_lowercase().contains("drop") || sql.to_lowercase().contains("delete") {
                return Err("Dangerous SQL detected".to_string());
            }

            query_database(sql).await
        }

        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("╔══════════════════════════════════════════════╗");
    println!("║  MatrixCode 自定义工具注入示例                ║");
    println!("╚══════════════════════════════════════════════╝\n");

    // 1. 创建代理工具
    println!("📝 创建代理工具...\n");

    // 知识库搜索工具
    let search_tool = ProxyTool::new(
        ToolDefinition {
            name: "search_knowledge".to_string(),
            description: "搜索内部知识库".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索关键词"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量",
                        "default": 10
                    }
                },
                "required": ["query"]
            }),
            ..Default::default()
        },
        ProxyMetadata {
            tool_type: "search".to_string(),
            endpoint: Some("http://internal-api:8080/search".to_string()),
            timeout_ms: 5000,
            custom: Some(json!({
                "auth_required": true,
                "cache_enabled": true
            })),
        },
    );

    // 数据库查询工具
    let database_tool = ProxyTool::new(
        ToolDefinition {
            name: "query_database".to_string(),
            description: "查询内部数据库（只读）".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sql": {
                        "type": "string",
                        "description": "SELECT 查询语句"
                    }
                },
                "required": ["sql"]
            }),
            ..Default::default()
        },
        ProxyMetadata {
            tool_type: "database".to_string(),
            endpoint: Some("postgresql://internal-db:5432".to_string()),
            timeout_ms: 10000,
            custom: Some(json!({
                "read_only": true,
                "max_rows": 1000
            })),
        },
    );

    println!("✅ 已创建 2 个代理工具:");
    println!("   - search_knowledge (搜索知识库)");
    println!("   - query_database (查询数据库)\n");

    // 2. 模拟 Agent 运行（简化版）
    println!("🤖 模拟 Agent 运行...\n");

    // 模拟场景 1: 大模型选择调用搜索工具
    println!("━━━━━━ 场景 1: 搜索知识库 ━━━━━━");
    let event = AgentEvent {
        event_type: EventType::ProxyToolRequest,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        data: Some(EventData::ProxyToolRequest {
            request_id: "req-001".to_string(),
            tool_name: "search_knowledge".to_string(),
            tool_input: json!({
                "query": "工具",
                "limit": 5
            }),
            metadata: search_tool.metadata().clone(),
        }),
    };

    if let Some(EventData::ProxyToolRequest {
        request_id: _,
        tool_name,
        tool_input,
        metadata,
        ..
    }) = event.data
    {
        let result = execute_custom_tool(&tool_name, tool_input.clone(), &metadata).await;

        match result {
            Ok(output) => {
                println!("✅ 执行成功:");
                println!("📤 结果: {}\n", output);
            }
            Err(e) => {
                println!("❌ 执行失败: {}\n", e);
            }
        }
    }

    // 模拟场景 2: 大模型选择调用数据库工具
    println!("━━━━━━ 场景 2: 查询数据库 ━━━━━━");
    let event = AgentEvent {
        event_type: EventType::ProxyToolRequest,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        data: Some(EventData::ProxyToolRequest {
            request_id: "req-002".to_string(),
            tool_name: "query_database".to_string(),
            tool_input: json!({
                "sql": "SELECT * FROM users LIMIT 5"
            }),
            metadata: database_tool.metadata().clone(),
        }),
    };

    if let Some(EventData::ProxyToolRequest {
        request_id: _,
        tool_name,
        tool_input,
        metadata,
        ..
    }) = event.data
    {
        let result = execute_custom_tool(&tool_name, tool_input.clone(), &metadata).await;

        match result {
            Ok(output) => {
                println!("✅ 执行成功:");
                println!("📤 结果: {}\n", output);
            }
            Err(e) => {
                println!("❌ 执行失败: {}\n", e);
            }
        }
    }

    // 模拟场景 3: 错误处理（危险 SQL）
    println!("━━━━━━ 场景 3: 错误处理 ━━━━━━");
    let event = AgentEvent {
        event_type: EventType::ProxyToolRequest,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        data: Some(EventData::ProxyToolRequest {
            request_id: "req-003".to_string(),
            tool_name: "query_database".to_string(),
            tool_input: json!({
                "sql": "DROP TABLE users"
            }),
            metadata: database_tool.metadata().clone(),
        }),
    };

    if let Some(EventData::ProxyToolRequest {
        request_id: _,
        tool_name,
        tool_input,
        metadata,
        ..
    }) = event.data
    {
        let result = execute_custom_tool(&tool_name, tool_input.clone(), &metadata).await;

        match result {
            Ok(output) => {
                println!("✅ 执行成功:");
                println!("📤 结果: {}\n", output);
            }
            Err(e) => {
                println!("❌ 执行失败: {}\n", e);
            }
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✨ 示例完成！\n");

    println!("📚 关键概念:");
    println!("   1. ProxyTool 定义工具，外部系统执行");
    println!("   2. Agent 通过事件通知调用方");
    println!("   3. 调用方自行执行工具逻辑");
    println!("   4. 通过 channel 返回结果给 Agent\n");

    println!("💡 下一步:");
    println!("   - 查看文档: docs/CUSTOM_TOOLS.md");
    println!("   - 查看源码: core/src/tools/toolproxy.rs");
    println!("   - 集成到你的应用中\n");

    Ok(())
}
