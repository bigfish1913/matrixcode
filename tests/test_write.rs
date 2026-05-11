use code_agent::tools::write::WriteTool;
use code_agent::tools::Tool;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn test_write_new_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.txt");

    let tool = WriteTool;
    let result = tool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "content": "hello world"
        }))
        .await
        .unwrap();

    assert!(result.contains("Successfully wrote"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
}

#[tokio::test]
async fn test_write_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a/b/c/test.txt");

    let tool = WriteTool;
    tool.execute(json!({
        "path": path.to_str().unwrap(),
        "content": "nested"
    }))
    .await
    .unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "nested");
}

#[tokio::test]
async fn test_write_missing_params() {
    let tool = WriteTool;
    assert!(tool.execute(json!({})).await.is_err());
    assert!(tool.execute(json!({"path": "/tmp/x"})).await.is_err());
}

#[tokio::test]
async fn test_write_definition() {
    let tool = WriteTool;
    let def = tool.definition();
    assert_eq!(def.name, "write");
    assert!(def.parameters["required"]
        .as_array()
        .unwrap()
        .contains(&json!("path")));
    assert!(def.parameters["required"]
        .as_array()
        .unwrap()
        .contains(&json!("content")));
}
