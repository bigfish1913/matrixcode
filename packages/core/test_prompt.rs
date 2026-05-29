use matrixcode_core::prompt::{build_static_system_prompt_with_codegraph, PromptProfile};

fn main() {
    // Test with CodeGraph
    let prompt_with_codegraph = build_static_system_prompt_with_codegraph(PromptProfile::Default, true);
    
    // Verify it contains CodeGraph-specific guidance
    assert!(prompt_with_codegraph.contains("code_search（优先，比 grep 快 10-100 倍）"), 
            "Should contain CodeGraph-specific TOOL_DECISION");
    assert!(prompt_with_codegraph.contains("code_callers/callees（优先）"), 
            "Should contain CodeGraph-specific DEBUGGING");
    
    println!("✅ With CodeGraph: Contains specific guidance");
    
    // Test without CodeGraph
    let prompt_without_codegraph = build_static_system_prompt_with_codegraph(PromptProfile::Default, false);
    
    // Verify it contains generic guidance
    assert!(prompt_without_codegraph.contains("查看工具列表中的符号搜索工具（如有）或用 grep"), 
            "Should contain generic TOOL_DECISION");
    assert!(prompt_without_codegraph.contains("使用专用符号搜索工具（如有）或 grep"), 
            "Should contain generic DEBUGGING");
    
    println!("✅ Without CodeGraph: Contains generic guidance");
    
    // Verify CodeGraph-specific guidance is NOT in generic version
    assert!(!prompt_without_codegraph.contains("code_search（优先，比 grep 快 10-100 倍）"), 
            "Should NOT contain CodeGraph-specific guidance");
    
    println!("✅ Generic version does not contain CodeGraph-specific text");
    
    println!("\n🎉 All tests passed! Dynamic prompt construction works correctly.");
}
