use code_agent::tools::webfetch::WebFetchTool;
use code_agent::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_macau_websites() {
    let tool = WebFetchTool;
    
    println!("\n=== 测试访问澳门网站 ===\n");
    
    // 测试澳门政府网站
    println!("测试 1: 澳门政府门户网站");
    match tool.execute(json!({
        "url": "https://www.gov.mo",
        "max_length": 500
    })).await {
        Ok(content) => {
            println!("✓ 成功获取内容 (前 500 字符):");
            println!("{}\n", content);
        }
        Err(e) => println!("✗ 错误: {}\n", e),
    }
    
    // 测试澳门气象局
    println!("测试 2: 澳门气象局");
    match tool.execute(json!({
        "url": "https://www.smg.gov.mo",
        "max_length": 500
    })).await {
        Ok(content) => {
            println!("✓ 成功获取内容 (前 500 字符):");
            println!("{}\n", content);
        }
        Err(e) => println!("✗ 错误: {}\n", e),
    }
    
    // 测试澳门大学
    println!("测试 3: 澳门大学");
    match tool.execute(json!({
        "url": "https://www.um.edu.mo",
        "max_length": 500
    })).await {
        Ok(content) => {
            println!("✓ 成功获取内容 (前 500 字符):");
            println!("{}\n", content);
        }
        Err(e) => println!("✗ 错误: {}\n", e),
    }
    
    println!("=== 测试完成 ===\n");
}