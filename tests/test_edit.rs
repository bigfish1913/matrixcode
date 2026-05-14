use matrixcode::tools::edit::EditTool;
use matrixcode::tools::Tool;
use serde_json::json;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_edit_replaces_string() {
    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "hello world").unwrap();

    let tool = EditTool;
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "old_string": "world",
            "new_string": "rust"
        }))
        .await
        .unwrap();

    assert!(result.contains("Successfully edited"));
    assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "hello rust");
}

#[tokio::test]
async fn test_edit_old_string_not_found() {
    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "hello world").unwrap();

    let tool = EditTool;
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "old_string": "nonexistent",
            "new_string": "replacement"
        }))
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_edit_multiple_matches_fails() {
    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "aaa bbb aaa").unwrap();

    let tool = EditTool;
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "old_string": "aaa",
            "new_string": "ccc"
        }))
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("2 times"));
}

#[tokio::test]
async fn test_edit_missing_params() {
    let tool = EditTool;
    assert!(tool.execute(json!({})).await.is_err());
    assert!(tool
        .execute(json!({"path": "/tmp/x", "old_string": "a"}))
        .await
        .is_err());
}

#[tokio::test]
async fn test_edit_definition() {
    let tool = EditTool;
    let def = tool.definition();
    assert_eq!(def.name, "edit");
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("path")));
    assert!(required.contains(&json!("old_string")));
    assert!(required.contains(&json!("new_string")));
}
