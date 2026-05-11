use code_agent::tools::Tool;
use code_agent::tools::glob::GlobTool;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn create_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.rs"), "").unwrap();
    fs::write(dir.path().join("b.rs"), "").unwrap();
    fs::write(dir.path().join("c.txt"), "").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/d.rs"), "").unwrap();
    fs::write(dir.path().join("sub/e.md"), "").unwrap();
    dir
}

#[tokio::test]
async fn test_glob_definition() {
    let tool = GlobTool;
    let def = tool.definition();
    assert_eq!(def.name, "glob");
    assert!(
        def.parameters["required"]
            .as_array()
            .unwrap()
            .contains(&json!("pattern"))
    );
}

#[tokio::test]
async fn test_glob_missing_pattern() {
    let tool = GlobTool;
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_glob_top_level_pattern() {
    let dir = create_test_dir();
    let tool = GlobTool;
    let result = tool
        .execute(json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("a.rs"));
    assert!(result.contains("b.rs"));
    assert!(!result.contains("c.txt"));
    assert!(!result.contains("d.rs"), "top-level * should not descend");
}

#[tokio::test]
async fn test_glob_recursive_double_star() {
    let dir = create_test_dir();
    let tool = GlobTool;
    let result = tool
        .execute(json!({
            "pattern": "**/*.rs",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("a.rs"));
    assert!(result.contains("b.rs"));
    assert!(result.contains("d.rs"));
    assert!(!result.contains("c.txt"));
    assert!(!result.contains("e.md"));
}

#[tokio::test]
async fn test_glob_no_matches() {
    let dir = create_test_dir();
    let tool = GlobTool;
    let result = tool
        .execute(json!({
            "pattern": "*.zzz",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert_eq!(result, "No files matched.");
}

#[tokio::test]
async fn test_glob_skips_ignored_dirs() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("node_modules")).unwrap();
    fs::write(dir.path().join("node_modules/junk.rs"), "").unwrap();
    fs::write(dir.path().join("keep.rs"), "").unwrap();

    let tool = GlobTool;
    let result = tool
        .execute(json!({
            "pattern": "**/*.rs",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("keep.rs"));
    assert!(!result.contains("junk.rs"));
}
