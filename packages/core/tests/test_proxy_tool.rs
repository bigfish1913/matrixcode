//! Test proxy tool chain: Agent → Event → External execution → Response

use matrixcode_core::{
    event::{AgentEvent, EventData, EventType},
    tools::{
        proxy::{ProxyMetadata, ProxyTool, ProxyToolRequest, ProxyToolResponse},
        ToolDefinition,
        Tool,
    },
};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test ProxyTool creation and metadata
#[test]
fn test_proxy_tool_creation() {
    let definition = ToolDefinition {
        name: "image_search".to_string(),
        description: "搜索网络图片".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "搜索关键词"},
                "max_results": {"type": "integer", "default": 5}
            },
            "required": ["query"]
        }),
        ..Default::default()
    };

    let metadata = ProxyMetadata {
        tool_type: "image_search".to_string(),
        endpoint: None,
        timeout_ms: 30000,
        custom: Some(json!({"platforms": ["unsplash", "pexels", "pixabay"]})),
    };

    let proxy_tool = ProxyTool::new(definition.clone(), metadata.clone());

    // Verify definition
    assert_eq!(proxy_tool.definition().name, "image_search");
    assert_eq!(proxy_tool.definition().description, "搜索网络图片");

    // Verify metadata
    assert_eq!(proxy_tool.metadata().tool_type, "image_search");
    assert_eq!(proxy_tool.metadata().timeout_ms, 30000);
}

/// Test ProxyToolRequest/Response serialization
#[test]
fn test_proxy_tool_request_response_serialization() {
    let request_id = Uuid::new_v4().to_string();

    let request = ProxyToolRequest {
        request_id: request_id.clone(),
        tool_name: "image_search".to_string(),
        tool_input: json!({"query": "cats", "max_results": 5}),
        metadata: ProxyMetadata {
            tool_type: "image_search".to_string(),
            endpoint: None,
            timeout_ms: 30000,
            custom: None,
        },
    };

    // Serialize request
    let request_json = serde_json::to_string(&request).unwrap();
    assert!(request_json.contains("image_search"));
    assert!(request_json.contains("cats"));

    // Deserialize request
    let deserialized: ProxyToolRequest = serde_json::from_str(&request_json).unwrap();
    assert_eq!(deserialized.request_id, request_id);
    assert_eq!(deserialized.tool_name, "image_search");

    // Create and test response
    let response = ProxyToolResponse {
        request_id: request_id.clone(),
        result: json!({"success": true, "images": []}).to_string(),
        is_error: false,
    };

    let response_json = serde_json::to_string(&response).unwrap();
    let deserialized_response: ProxyToolResponse = serde_json::from_str(&response_json).unwrap();
    assert_eq!(deserialized_response.request_id, request_id);
    assert!(!deserialized_response.is_error);
}

/// Test full channel communication chain (Agent → Event → Response)
#[tokio::test]
async fn test_proxy_tool_channel_chain() {
    // Create channels
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(10);
    let (response_tx, mut response_rx) = mpsc::channel::<ProxyToolResponse>(10);

    // Simulate Agent sending ProxyToolRequest event
    let request_id = Uuid::new_v4().to_string();
    let event = AgentEvent::proxy_tool_request(
        request_id.clone(),
        "image_search".to_string(),
        json!({"query": "sunset", "max_results": 3}),
        ProxyMetadata {
            tool_type: "image_search".to_string(),
            endpoint: None,
            timeout_ms: 30000,
            custom: None,
        },
    );

    // Agent sends event
    event_tx.send(event).await.unwrap();

    // External handler receives event
    let received_event = event_rx.recv().await.unwrap();
    assert_eq!(received_event.event_type, EventType::ProxyToolRequest);

    // Extract request data from event
    if let Some(EventData::ProxyToolRequest {
        request_id: recv_id,
        tool_name,
        tool_input,
        metadata: _,
    }) = received_event.data
    {
        assert_eq!(recv_id, request_id);
        assert_eq!(tool_name, "image_search");
        assert_eq!(tool_input["query"], "sunset");

        // Simulate external execution (e.g., calling real API)
        let mock_result = json!({
            "success": true,
            "query": "sunset",
            "total": 3,
            "images": [
                {"url": "https://example.com/sunset1.jpg", "platform": "Unsplash"},
                {"url": "https://example.com/sunset2.jpg", "platform": "Pexels"},
                {"url": "https://example.com/sunset3.jpg", "platform": "Pixabay"}
            ]
        });

        // External handler sends response back
        let response = ProxyToolResponse {
            request_id: recv_id,
            result: mock_result.to_string(),
            is_error: false,
        };
        response_tx.send(response).await.unwrap();
    } else {
        panic!("Expected ProxyToolRequest event data");
    }

    // Agent receives response (simulating handle_proxy_tool receiving)
    let received_response = response_rx.recv().await.unwrap();
    assert_eq!(received_response.request_id, request_id);
    assert!(!received_response.is_error);

    // Parse result
    let result: serde_json::Value = serde_json::from_str(&received_response.result).unwrap();
    assert_eq!(result["success"], true);
    assert_eq!(result["total"], 3);
}

/// Test error handling in proxy tool chain
#[tokio::test]
async fn test_proxy_tool_error_chain() {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(10);
    let (response_tx, mut response_rx) = mpsc::channel::<ProxyToolResponse>(10);

    let request_id = Uuid::new_v4().to_string();

    // Agent sends request
    event_tx.send(AgentEvent::proxy_tool_request(
        request_id.clone(),
        "image_search".to_string(),
        json!({"query": ""}), // Empty query - should fail
        ProxyMetadata {
            tool_type: "image_search".to_string(),
            endpoint: None,
            timeout_ms: 30000,
            custom: None,
        },
    )).await.unwrap();

    // External handler receives and detects error
    let received_event = event_rx.recv().await.unwrap();
    if let Some(EventData::ProxyToolRequest { request_id: recv_id, tool_input, .. }) = received_event.data {
        let query = tool_input.get("query").and_then(|q| q.as_str()).unwrap_or("");

        // Empty query = error
        let response = if query.is_empty() {
            ProxyToolResponse {
                request_id: recv_id,
                result: json!({"error": "query is required"}).to_string(),
                is_error: true,
            }
        } else {
            ProxyToolResponse {
                request_id: recv_id,
                result: json!({"success": true}).to_string(),
                is_error: false,
            }
        };
        response_tx.send(response).await.unwrap();
    }

    // Agent receives error response
    let received_response = response_rx.recv().await.unwrap();
    assert!(received_response.is_error);

    let result: serde_json::Value = serde_json::from_str(&received_response.result).unwrap();
    assert!(result.get("error").is_some());
}