//! Integration test for CodeGraph tools

use matrixcode_core::tools::codegraph::{CodeGraphSearchTool, CodeGraphStatusTool};
use matrixcode_core::tools::Tool;
use std::path::PathBuf;

fn get_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[tokio::test]
async fn test_codegraph_search_tool_execute() {
    let project_path = get_project_path();

    let codegraph_dir = project_path.join(".codegraph");
    if !codegraph_dir.exists() {
        eprintln!("Skipping test: .codegraph directory not found at {}", codegraph_dir.display());
        return;
    }

    let tool = CodeGraphSearchTool::new(&project_path);
    let def = tool.definition();
    assert_eq!(def.name, "code_search");

    // Search for a common symbol
    let result = tool.execute(serde_json::json!({
        "pattern": "Agent",
        "limit": 5
    })).await;

    match result {
        Ok(output) => {
            println!("Search result: {}", output);
            assert!(!output.is_empty(), "Search should return results");
        }
        Err(e) => {
            println!("Search error (may be expected if no matching symbols): {}", e);
        }
    }
}

#[tokio::test]
async fn test_codegraph_status_tool_execute() {
    let project_path = get_project_path();

    let codegraph_dir = project_path.join(".codegraph");
    if !codegraph_dir.exists() {
        eprintln!("Skipping test: .codegraph directory not found at {}", codegraph_dir.display());
        return;
    }

    let tool = CodeGraphStatusTool::new(&project_path);
    let def = tool.definition();
    assert_eq!(def.name, "code_status");

    let result = tool.execute(serde_json::json!({})).await;

    match result {
        Ok(status) => {
            println!("Status: {}", status);
            // Check for Chinese or English output
            assert!(
                status.contains("node_count")
                || status.contains("节点数")
                || status.contains("initialized")
                || status.contains("文件数"),
                "Status should show index info"
            );
        }
        Err(e) => {
            println!("Status error: {}", e);
        }
    }
}

#[test]
fn test_generate_tools_prompt_with_codegraph() {
    use matrixcode_core::tools::generate_tools_prompt_with_path;

    let project_path = get_project_path();

    let prompt = generate_tools_prompt_with_path(Some(&project_path));

    println!("Tools prompt:\n{}", prompt);

    assert!(prompt.contains("code_search"), "Prompt should include code_search");
    assert!(prompt.contains("code_callers"), "Prompt should include code_callers");
    assert!(prompt.contains("code_callees"), "Prompt should include code_callees");
    assert!(prompt.contains("code_status"), "Prompt should include code_status");
}

#[test]
fn test_build_system_prompt_with_codegraph() {
    use matrixcode_core::prompt::{build_system_prompt_with_workflows, PromptProfile};

    let project_path = get_project_path();

    let prompt = build_system_prompt_with_workflows(
        &PromptProfile::Default,
        &[],
        None,
        None,
        Some(&project_path),
    );

    // Check if codegraph tools are mentioned in the prompt
    assert!(prompt.contains("code_search"), "System prompt should include code_search");
    assert!(prompt.contains("code_callers"), "System prompt should include code_callers");
}