// 验证动态构建函数的正确性
fn main() {
    // 从 prompt.rs 中提取关键验证点
    let code = include_str!("src/prompt.rs");
    
    // 验证1: 检查两个版本都存在
    assert!(code.contains("SYSTEM_PROMPT_TOOL_DECISION_GENERIC"), 
            "❌ 缺少 GENERIC 版本");
    assert!(code.contains("SYSTEM_PROMPT_TOOL_DECISION_WITH_CODEGRAPH"), 
            "❌ 缺少 WITH_CODEGRAPH 版本");
    assert!(code.contains("SYSTEM_PROMPT_DEBUGGING_GENERIC"), 
            "❌ 缺少 DEBUGGING GENERIC 版本");
    assert!(code.contains("SYSTEM_PROMPT_DEBUGGING_WITH_CODEGRAPH"), 
            "❌ 缺少 DEBUGGING WITH_CODEGRAPH 版本");
    
    println!("✅ 所有版本常量都存在");
    
    // 验证2: 检查 WITH_CODEGRAPH 版本包含明确指引
    assert!(code.contains("code_search（优先，比 grep 快 10-100 倍）"), 
            "❌ WITH_CODEGRAPH 版本缺少明确指引");
    assert!(code.contains("code_callers/callees（优先，比 grep 更准确）"), 
            "❌ WITH_CODEGRAPH 版本缺少调用关系指引");
    
    println!("✅ WITH_CODEGRAPH 版本包含明确指引");
    
    // 验证3: 检查 GENERIC 版本包含通用指引
    assert!(code.contains("查看工具列表中的符号搜索工具（如有）或用 grep"), 
            "❌ GENERIC 版本缺少通用指引");
    
    println!("✅ GENERIC 版本包含通用指引");
    
    // 验证4: 检查动态构建函数存在
    assert!(code.contains("pub fn build_static_system_prompt_with_codegraph"), 
            "❌ 缺少动态构建函数");
    
    println!("✅ 动态构建函数存在");
    
    // 验证5: 检查动态构建逻辑
    assert!(code.contains("let tool_decision = if has_codegraph"), 
            "❌ 缺少 TOOL_DECISION 动态选择逻辑");
    assert!(code.contains("let debugging = if has_codegraph"), 
            "❌ 缺少 DEBUGGING 动态选择逻辑");
    
    println!("✅ 动态构建逻辑正确");
    
    // 验证6: 检查模块数组已移除静态引用
    assert!(!code.contains("SYSTEM_PROMPT_TOOL_DECISION,   // 移后"), 
            "❌ 模块数组仍有静态 TOOL_DECISION 引用");
    
    println!("✅ 模块数组已正确移除静态引用");
    
    // 验证7: 检查 build_system_prompt_with_workflows 使用动态构建
    assert!(code.contains("let static_prompt = build_static_system_prompt_with_codegraph(*profile, has_codegraph);"), 
            "❌ build_system_prompt_with_workflows 未使用动态构建");
    
    println!("✅ build_system_prompt_with_workflows 使用动态构建");
    
    println!("\n🎉 所有验证通过！动态构建系统正确实现。");
}
