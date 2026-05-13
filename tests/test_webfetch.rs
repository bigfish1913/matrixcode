use code_agent::tools::webfetch::WebFetchTool;
use code_agent::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_webfetch_definition() {
    let tool = WebFetchTool;
    let def = tool.definition();
    assert_eq!(def.name, "webfetch");
    assert!(def.parameters["required"]
        .as_array()
        .unwrap()
        .contains(&json!("url")));
}

#[tokio::test]
async fn test_webfetch_missing_url() {
    let tool = WebFetchTool;
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_webfetch_real_url() {
    let tool = WebFetchTool;
    // 使用一个稳定且响应快速的测试 URL
    let result = tool.execute(json!({
        "url": "https://httpbin.org/get"
    })).await;
    
    assert!(result.is_ok(), "Failed to fetch URL: {:?}", result.err());
    let content = result.unwrap();
    assert!(!content.is_empty(), "Response should not be empty");
    // httpbin.org/get 返回 JSON，应该包含请求信息
    assert!(content.contains("\"url\""), "Response should contain URL info");
}

#[tokio::test]
async fn test_webfetch_with_max_length() {
    let tool = WebFetchTool;
    let result = tool.execute(json!({
        "url": "https://httpbin.org/get",
        "max_length": 50
    })).await;
    
    assert!(result.is_ok());
    let content = result.unwrap();
    // 内容应该被截断，并包含 truncation 标记
    assert!(content.contains("truncated") || content.len() < 200, 
            "Content should be truncated or small enough");
}

#[tokio::test]
async fn test_webfetch_invalid_url() {
    let tool = WebFetchTool;
    let result = tool.execute(json!({
        "url": "not-a-valid-url"
    })).await;
    
    assert!(result.is_err(), "Invalid URL should return error");
}

#[tokio::test]
async fn test_webfetch_404() {
    let tool = WebFetchTool;
    let result = tool.execute(json!({
        "url": "https://httpbin.org/status/404"
    })).await;
    
    assert!(result.is_err(), "404 should return error");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("404"), "Error should mention 404 status");
}
