use matrixcode_core::tools::Tool;
use matrixcode_core::tools::todo_write::TodoWriteTool;
use serde_json::json;

#[tokio::test]
async fn test_todo_write_definition() {
    let def = TodoWriteTool.definition();
    assert_eq!(def.name, "todo_write");
    assert!(
        def.parameters["required"]
            .as_array()
            .unwrap()
            .contains(&json!("todos"))
    );
}

#[tokio::test]
async fn test_todo_write_renders_all_statuses() {
    let out = TodoWriteTool
        .execute(json!({
            "todos": [
                {"content": "Write parser", "activeForm": "Writing parser", "status": "completed"},
                {"content": "Add tests", "activeForm": "Adding tests", "status": "in_progress"},
                {"content": "Document API", "activeForm": "Documenting API", "status": "pending"}
            ]
        }))
        .await
        .unwrap();

    assert!(out.starts_with("Todos (3):"));
    assert!(out.contains("[x] Write parser"));
    assert!(out.contains("[~] Adding tests"));
    assert!(out.contains("[ ] Document API"));
}

#[tokio::test]
async fn test_todo_write_rejects_multiple_in_progress() {
    let err = TodoWriteTool
        .execute(json!({
            "todos": [
                {"content": "a", "activeForm": "A", "status": "in_progress"},
                {"content": "b", "activeForm": "B", "status": "in_progress"}
            ]
        }))
        .await
        .expect_err("should reject 2 in_progress");
    assert!(err.to_string().contains("at most one"));
}

#[tokio::test]
async fn test_todo_write_rejects_invalid_status() {
    let err = TodoWriteTool
        .execute(json!({
            "todos": [{"content": "x", "activeForm": "X", "status": "done"}]
        }))
        .await
        .expect_err("should reject unknown status");
    assert!(err.to_string().contains("invalid status"));
}

#[tokio::test]
async fn test_todo_write_empty_list_clears() {
    let out = TodoWriteTool.execute(json!({"todos": []})).await.unwrap();
    assert!(out.contains("cleared"));
}

#[tokio::test]
async fn test_todo_write_missing_field() {
    let err = TodoWriteTool
        .execute(json!({
            "todos": [{"content": "x", "status": "pending"}]
        }))
        .await
        .expect_err("missing activeForm should fail");
    assert!(err.to_string().contains("activeForm"));
}
