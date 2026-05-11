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
