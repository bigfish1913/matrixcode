use matrixcode_core::tools::generate_tools_prompt_with_path;
use matrixcode_core::tools::codegraph::should_inject_codegraph_tools;
use matrixcode_core::prompt::{build_system_prompt_with_workflows, PromptProfile};
use std::path::PathBuf;

fn main() {
    let project_path = PathBuf::from(".");

    println!("=== CodeGraph 状态检查 ===");
    let should_inject = should_inject_codegraph_tools(&project_path);
    println!("Should inject: {}", should_inject);
    println!();

    // 生成工具列表
    println!("=== 工具列表（前1000字符）===");
    let tools_prompt = generate_tools_prompt_with_path(Some(&project_path));
    println!("{}", &tools_prompt[..tools_prompt.len().min(1000)]);
    println!();

    // 查找 CodeGraph 工具
    println!("=== CodeGraph 工具检查 ===");
    for line in tools_prompt.lines() {
        if line.contains("code_") {
            println!("{}", line);
        }
    }
    println!();

    // 统计优先工具
    println!("=== [优先] 标记统计 ===");
    let priority_count = tools_prompt.matches("[优先]").count();
    println!("包含 [优先] 标记的工具数量: {}", priority_count);
    for line in tools_prompt.lines() {
        if line.contains("[优先]") {
            println!("{}", line);
        }
    }
    println!();

    // 完整 system prompt
    println!("=== System Prompt 检查 ===");
    let system_prompt = build_system_prompt_with_workflows(
        &PromptProfile::Default,
        &[],
        None,
        None,
        Some(&project_path),
    );

    // 查找 CODEGRAPH 规则
    if let Some(start) = system_prompt.find("CodeGraph") {
        println!("找到 CodeGraph 引用位置: {}", start);
        let snippet = &system_prompt[start..system_prompt.len().min(start + 300)];
        println!("片段:\n{}", snippet);
    } else {
        println!("未找到 CodeGraph 相关内容");
    }

    // 查找 "可用工具：" 部分
    if let Some(start) = system_prompt.find("可用工具：") {
        let end = system_prompt[start..].find("\n\n").map(|i| start + i).unwrap_or(system_prompt.len());
        let tools_section = &system_prompt[start..end];
        println!("\n工具列表部分:");
        for (i, line) in tools_section.lines().enumerate().take(15) {
            println!("{}. {}", i + 1, line);
        }
    }
}