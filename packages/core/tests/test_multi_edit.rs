use matrixcode_core::tools::Tool;
use matrixcode_core::tools::multi_edit::MultiEditTool;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn tmp_file(content: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("f.txt");
    fs::write(&path, content).unwrap();
    (dir, path)
}

#[tokio::test]
async fn test_multi_edit_definition() {
    let tool = MultiEditTool;
    let def = tool.definition();
    assert_eq!(def.name, "multi_edit");
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("path")));
    assert!(required.contains(&json!("edits")));
}

#[tokio::test]
async fn test_multi_edit_applies_all_sequentially() {
    let (_d, path) = tmp_file("alpha beta gamma");
    let out = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "alpha", "new_string": "A"},
                {"old_string": "beta", "new_string": "B"},
                {"old_string": "gamma", "new_string": "C"}
            ]
        }))
        .await
        .unwrap();
    assert!(out.contains("Applied 3 edit(s)"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "A B C");
}

#[tokio::test]
async fn test_multi_edit_sees_prior_edits() {
    let (_d, path) = tmp_file("foo");
    MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "foo", "new_string": "bar"},
                {"old_string": "bar", "new_string": "baz"}
            ]
        }))
        .await
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "baz");
}

#[tokio::test]
async fn test_multi_edit_atomic_on_failure() {
    let (_d, path) = tmp_file("hello world");
    let err = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "hello", "new_string": "hi"},
                {"old_string": "nonexistent", "new_string": "x"}
            ]
        }))
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("not found"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "hello world",
        "file must not be modified when any edit fails"
    );
}

#[tokio::test]
async fn test_multi_edit_rejects_non_unique() {
    let (_d, path) = tmp_file("aa aa aa");
    let err = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "aa", "new_string": "b"}
            ]
        }))
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("found 3 times"));
}

#[tokio::test]
async fn test_multi_edit_empty_edits() {
    let (_d, path) = tmp_file("x");
    let err = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": []
        }))
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("at least one"));
}

#[tokio::test]
async fn test_multi_edit_crlf_file_with_lf_search() {
    // Test: file uses CRLF, but old_string uses LF (common AI input case)
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.txt");
    // Write file with CRLF line endings (Windows style)
    fs::write(&path, "line1\r\nline2\r\nline3\r\n").unwrap();

    let result = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "line1\nline2", "new_string": "new1\nnew2"},
                {"old_string": "line3", "new_string": "new3"}
            ]
        }))
        .await
        .unwrap();

    assert!(result.contains("Applied 2 edit(s)"));
    // File should still have CRLF after edit
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("\r\n"), "File should retain CRLF line endings");
    assert_eq!(content, "new1\r\nnew2\r\nnew3\r\n");
}

#[tokio::test]
async fn test_multi_edit_lf_file_with_crlf_search() {
    // Test: file uses LF, but old_string uses CRLF
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.txt");
    // Write file with LF line endings (Unix style)
    fs::write(&path, "line1\nline2\nline3\n").unwrap();

    let result = MultiEditTool
        .execute(json!({
            "path": path.to_str().unwrap(),
            "edits": [
                {"old_string": "line1\r\nline2", "new_string": "new1\r\nnew2"},
                {"old_string": "line3", "new_string": "new3"}
            ]
        }))
        .await
        .unwrap();

    assert!(result.contains("Applied 2 edit(s)"));
    // File should still have LF after edit
    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("\r\n"), "File should retain LF line endings");
    assert_eq!(content, "new1\nnew2\nnew3\n");
}
