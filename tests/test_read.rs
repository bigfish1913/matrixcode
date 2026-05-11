use code_agent::tools::read::ReadTool;
use code_agent::tools::Tool;
use serde_json::json;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_read_full_file() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "line 1").unwrap();
    writeln!(tmp, "line 2").unwrap();
    writeln!(tmp, "line 3").unwrap();

    let tool = ReadTool;
    let result = tool
        .execute(json!({"path": tmp.path().to_str().unwrap()}))
        .await
        .unwrap();

    assert!(result.contains("line 1"));
    assert!(result.contains("line 2"));
    assert!(result.contains("line 3"));
}

#[tokio::test]
async fn test_read_with_offset_and_limit() {
    let mut tmp = NamedTempFile::new().unwrap();
    for i in 1..=10 {
        writeln!(tmp, "line {}", i).unwrap();
    }

    let tool = ReadTool;
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "offset": 2,
            "limit": 3
        }))
        .await
        .unwrap();

    assert!(result.contains("line 3"));
    assert!(result.contains("line 4"));
    assert!(result.contains("line 5"));
    assert!(!result.contains("| line 1"));
    assert!(!result.contains("| line 6"));
}

#[tokio::test]
async fn test_read_missing_path_param() {
    let tool = ReadTool;
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    let tool = ReadTool;
    let result = tool
        .execute(json!({"path": "/nonexistent/file.txt"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_definition() {
    let tool = ReadTool;
    let def = tool.definition();
    assert_eq!(def.name, "read");
    assert!(!def.description.is_empty());
    assert!(def.parameters["properties"]["path"].is_object());
}
