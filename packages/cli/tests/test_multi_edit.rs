use matrixcode::tools::Tool;
use matrixcode::tools::multi_edit::MultiEditTool;
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
