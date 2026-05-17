use matrixcode_core::tools::search::SearchTool;
use matrixcode_core::tools::Tool;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn create_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("foo.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    fs::write(dir.path().join("bar.txt"), "some text\nhello world\n").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/baz.rs"), "fn test() {}\n").unwrap();
    dir
}

#[tokio::test]
async fn test_search_finds_pattern() {
    let dir = create_test_dir();

    let tool = SearchTool;
    let result = tool
        .execute(json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("hello"));
}

#[tokio::test]
async fn test_search_no_matches() {
    let dir = create_test_dir();

    let tool = SearchTool;
    let result = tool
        .execute(json!({
            "pattern": "zzz_nonexistent",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert_eq!(result, "No matches found.");
}

#[tokio::test]
async fn test_search_with_glob() {
    let dir = create_test_dir();

    let tool = SearchTool;
    let result = tool
        .execute(json!({
            "pattern": "fn",
            "path": dir.path().to_str().unwrap(),
            "glob": "*.rs"
        }))
        .await
        .unwrap();

    assert!(result.contains("fn"));
    assert!(!result.contains("bar.txt"));
}

#[tokio::test]
async fn test_search_missing_pattern() {
    let tool = SearchTool;
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_regex_pattern() {
    let dir = create_test_dir();

    let tool = SearchTool;
    let result = tool
        .execute(json!({
            "pattern": "fn \\w+\\(",
            "path": dir.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("fn main("));
    assert!(result.contains("fn test("));
}

#[tokio::test]
async fn test_search_definition() {
    let tool = SearchTool;
    let def = tool.definition();
    assert_eq!(def.name, "search");
    assert!(def.parameters["required"]
        .as_array()
        .unwrap()
        .contains(&json!("pattern")));
}
