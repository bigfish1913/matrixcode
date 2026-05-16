use matrixcode::tools::Tool;
use matrixcode::tools::bash::BashTool;
use serde_json::json;

#[tokio::test]
async fn test_bash_definition() {
    let tool = BashTool;
    let def = tool.definition();
    assert_eq!(def.name, "bash");
    assert!(
        def.parameters["required"]
            .as_array()
            .unwrap()
            .contains(&json!("command"))
    );
}

#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool;
    let out = tool
        .execute(json!({"command": "echo hello-bash"}))
        .await
        .unwrap();
    assert!(out.contains("hello-bash"));
}

#[tokio::test]
async fn test_bash_nonzero_exit() {
    let tool = BashTool;
    let out = tool
        .execute(json!({"command": "sh -c 'echo oops >&2; exit 3'"}))
        .await
        .unwrap();
    assert!(out.contains("[exit 3]"));
    assert!(out.contains("oops"));
}

#[tokio::test]
async fn test_bash_timeout() {
    let tool = BashTool;
    let result = tool
        .execute(json!({"command": "sleep 5", "timeout_ms": 200}))
        .await;
    let err = result.expect_err("should have timed out");
    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn test_bash_refuses_destructive() {
    let tool = BashTool;
    let err = tool
        .execute(json!({"command": "rm -rf /"}))
        .await
        .expect_err("should be refused");
    assert!(err.to_string().contains("refused"));
}

#[tokio::test]
async fn test_bash_missing_command() {
    let tool = BashTool;
    assert!(tool.execute(json!({})).await.is_err());
}
