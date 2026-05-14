use matrixcode::tools::Tool;
use matrixcode::tools::ls::LsTool;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_ls_definition() {
    let def = LsTool.definition();
    assert_eq!(def.name, "ls");
}

#[tokio::test]
async fn test_ls_lists_dirs_and_files() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "hi").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();

    let out = LsTool
        .execute(json!({"path": dir.path().to_str().unwrap()}))
        .await
        .unwrap();

    assert!(out.contains("sub/"));
    assert!(out.contains("a.txt"));
    assert!(out.contains("(2 B)"));
    // dirs should appear before files
    let sub_pos = out.find("sub/").unwrap();
    let file_pos = out.find("a.txt").unwrap();
    assert!(sub_pos < file_pos);
}

#[tokio::test]
async fn test_ls_empty_dir() {
    let dir = TempDir::new().unwrap();
    let out = LsTool
        .execute(json!({"path": dir.path().to_str().unwrap()}))
        .await
        .unwrap();
    assert!(out.starts_with("(empty)"));
}

#[tokio::test]
async fn test_ls_missing_dir() {
    assert!(
        LsTool
            .execute(json!({"path": "/nonexistent/xxx/yyy"}))
            .await
            .is_err()
    );
}
