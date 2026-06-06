use matrixcode_core::tools::Tool;
use matrixcode_core::tools::edit::EditTool;
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
    assert!(
        tool.execute(json!({"path": "/tmp/x", "old_string": "a"}))
            .await
            .is_err()
    );
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

#[tokio::test]
async fn test_edit_crlf_file_with_lf_search() {
    // Test: file uses CRLF, but old_string uses LF (common AI input case)
    let mut tmp = NamedTempFile::new().unwrap();
    // Write file with CRLF line endings (Windows style)
    write!(tmp, "line1\r\nline2\r\nline3\r\n").unwrap();

    let tool = EditTool;
    // AI sends LF-style old_string
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "old_string": "line1\nline2",  // LF only
            "new_string": "new1\nnew2"      // LF only
        }))
        .await
        .unwrap();

    assert!(result.contains("Successfully edited"));
    // File should still have CRLF after edit
    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(content.contains("\r\n"), "File should retain CRLF line endings");
    assert!(content.contains("new1"));
    assert!(content.contains("new2"));
}

#[tokio::test]
async fn test_edit_lf_file_with_crlf_search() {
    // Test: file uses LF, but old_string uses CRLF
    let mut tmp = NamedTempFile::new().unwrap();
    // Write file with LF line endings (Unix style)
    write!(tmp, "line1\nline2\nline3\n").unwrap();

    let tool = EditTool;
    // User sends CRLF-style old_string
    let result = tool
        .execute(json!({
            "path": tmp.path().to_str().unwrap(),
            "old_string": "line1\r\nline2",  // CRLF
            "new_string": "new1\r\nnew2"      // CRLF
        }))
        .await
        .unwrap();

    assert!(result.contains("Successfully edited"));
    // File should still have LF after edit
    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(!content.contains("\r\n"), "File should retain LF line endings");
    assert!(content.contains("new1"));
    assert!(content.contains("new2"));
}
