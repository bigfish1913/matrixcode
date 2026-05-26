//! Debug test for system prompt

use matrixcode_core::prompt::{build_system_prompt_with_workflows, PromptProfile};
use std::path::PathBuf;

fn get_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[test]
fn test_print_full_system_prompt() {
    let project_path = get_project_path();

    let prompt = build_system_prompt_with_workflows(
        &PromptProfile::Default,
        &[],
        None,
        None,
        Some(&project_path),
    );

    // Print the full prompt to see its structure
    println!("=== FULL SYSTEM PROMPT ===");
    println!("{}", prompt);
    println!("=== END PROMPT ===");

    // Check key sections
    println!("\n=== CODEGRAPH SECTION CHECK ===");
    if prompt.contains("CodeGraph") {
        println!("✓ Contains 'CodeGraph'");
        // Find and print the CodeGraph section
        let start = prompt.find("CodeGraph").unwrap();
        let end = start + 500.min(prompt.len() - start);
        println!("CodeGraph section:\n{}", &prompt[start..end]);
    } else {
        println!("❌ Does NOT contain 'CodeGraph'");
    }

    println!("\n=== TOOLS SECTION CHECK ===");
    if prompt.contains("codegraph_search") {
        println!("✓ Contains 'codegraph_search'");
    } else {
        println!("❌ Does NOT contain 'codegraph_search'");
    }

    if prompt.contains("codegraph_callers") {
        println!("✓ Contains 'codegraph_callers'");
    } else {
        println!("❌ Does NOT contain 'codegraph_callers'");
    }

    // Check if ls and grep are present too
    println!("\n=== OTHER TOOLS CHECK ===");
    if prompt.contains("- ls:") {
        println!("✓ Contains 'ls' tool");
    }
    if prompt.contains("- grep:") || prompt.contains("- search:") {
        println!("✓ Contains grep/search tools");
    }
}