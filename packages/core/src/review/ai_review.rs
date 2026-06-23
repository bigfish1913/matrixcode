//! AI review implementation for pre-write quality assurance.

use anyhow::Result;
use tokio::time::{timeout, Duration};
use crate::providers::{Provider, ChatRequest, ChatResponse, Message, MessageContent, Role, ContentBlock};
use super::{ImpactAnalysis, PreWriteReviewInput, PreWriteReviewResult, ReviewContext};

const REVIEW_TIMEOUT_SECS: u64 = 30;

pub async fn perform_review(
    provider: &dyn Provider,
    input: &PreWriteReviewInput,
) -> Result<PreWriteReviewResult> {
    let prompt = build_review_prompt(input);
    
    // Build chat request
    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }],
        tools: vec![],
        system: None,
        think: false,
        max_tokens: 2000,
        server_tools: vec![],
        enable_caching: false,
    };
    
    // Call AI with timeout
    let review_timeout = Duration::from_secs(REVIEW_TIMEOUT_SECS);
    let response = timeout(review_timeout, provider.chat(request))
        .await
        .map_err(|_| anyhow::anyhow!("Review timeout after {} seconds", REVIEW_TIMEOUT_SECS))??;
    
    // Extract text from response
    let text = extract_response_text(&response);
    parse_review_result(&text)
}

fn extract_response_text(response: &ChatResponse) -> String {
    response.content.iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_review_prompt(input: &PreWriteReviewInput) -> String {
    let mut prompt_parts = Vec::new();
    
    // Header
    prompt_parts.push(format!("📋 Code Review: {}", input.file_path.display()));
    prompt_parts.push(format!("Tool: {}", input.tool_name));
    
    // Change summary
    if input.edit_info.is_some() {
        prompt_parts.push("\n📝 Change Type: Edit (modifying existing code)".to_string());
    } else if input.existing_content.is_none() {
        prompt_parts.push("\n📝 Change Type: New File".to_string());
    } else {
        prompt_parts.push("\n📝 Change Type: Overwrite (replacing entire file)".to_string());
    }
    
    // Code preview
    let code_preview = if input.new_content.len() > 5000 {
        format!("{}\n\n... (truncated, {} bytes total)", 
            &input.new_content[..5000], 
            input.new_content.len())
    } else {
        input.new_content.clone()
    };
    
    prompt_parts.push(format!("\n📄 New Content:\n```\n{}\n```", code_preview));
    
    // Existing content diff context
    if let Some(existing) = &input.existing_content {
        if existing != &input.new_content {
            prompt_parts.push("\n📁 Existing Content Summary:".to_string());
            prompt_parts.push(format!("  - Lines: {}", existing.lines().count()));
            prompt_parts.push(format!("  - Size: {} bytes", existing.len()));
            
            // Show key differences if it's an edit
            if let Some(edit) = &input.edit_info {
                let old_preview = if edit.old_string.len() > 200 {
                    format!("{}...", &edit.old_string[..200])
                } else {
                    edit.old_string.clone()
                };
                let new_preview = if edit.new_string.len() > 200 {
                    format!("{}...", &edit.new_string[..200])
                } else {
                    edit.new_string.clone()
                };
                prompt_parts.push(format!("\n  Removed:\n    {}", old_preview));
                prompt_parts.push(format!("\n  Added:\n    {}", new_preview));
            }
        }
    }
    
    // LSP diagnostics context
    add_lsp_context(&mut prompt_parts, &input.context);
    
    // CodeGraph context
    add_codegraph_context(&mut prompt_parts, &input.context);
    
    // Memory context
    add_memory_context(&mut prompt_parts, &input.context);
    
    // Review instructions
    prompt_parts.push("\n\n🔍 Review Instructions:".to_string());
    prompt_parts.push("Analyze the code for:".to_string());
    prompt_parts.push("  1. Quality: Naming, structure, readability, error handling".to_string());
    prompt_parts.push("  2. Security: Input validation, injection risks, sensitive data".to_string());
    prompt_parts.push("  3. Performance: Efficiency, memory usage, potential bottlenecks".to_string());
    prompt_parts.push("  4. Impact: Breaking changes, affected modules, dependencies".to_string());
    prompt_parts.push("  5. Practice: Consistency with project patterns, documentation".to_string());
    
    // Output format
    prompt_parts.push("\n\n📤 Output JSON:".to_string());
    prompt_parts.push(r#"{
  "overall_score": 85,
  "issues": [
    {"level": "Warning", "category": "Quality", "message": "Generic function name", "location": "line 10", "fix_example": null}
  ],
  "impact_analysis": {
    "affected_modules": [],
    "dependencies": [],
    "breaking_changes": false
  },
  "suggestions": ["Add unit tests"]
}"#.to_string());
    
    prompt_parts.push("\n\n📊 Score Guidelines:".to_string());
    prompt_parts.push("  - 90-100: Excellent, ready to write".to_string());
    prompt_parts.push("  - 70-89: Good with minor issues".to_string());
    prompt_parts.push("  - 60-69: Acceptable with warnings".to_string());
    prompt_parts.push("  - Below 60: Needs improvement".to_string());
    prompt_parts.push("\n⚠️ Critical issues (security, breaking changes) block write operation.".to_string());
    
    prompt_parts.join("\n")
}

fn add_lsp_context(parts: &mut Vec<String>, context: &ReviewContext) {
    if context.lsp_diagnostics.is_empty() {
        return;
    }
    
    parts.push("\n🔎 LSP Diagnostics:".to_string());
    for diag in &context.lsp_diagnostics {
        let location = diag.line.map(|l| format!("line {}", l)).unwrap_or_else(|| "unknown".to_string());
        parts.push(format!("  {} {}: {} ({})",
            diag.severity.icon(),
            diag.source.as_deref().unwrap_or("LSP"),
            diag.message,
            location
        ));
    }
}

fn add_codegraph_context(parts: &mut Vec<String>, context: &ReviewContext) {
    if context.symbols.is_empty() && context.callers.is_empty() && context.callees.is_empty() {
        return;
    }
    
    parts.push("\n🔗 CodeGraph Analysis:".to_string());
    
    // Symbols in file
    if !context.symbols.is_empty() {
        parts.push("  Symbols:".to_string());
        for sym in &context.symbols {
            let sig_preview = sym.signature.as_deref()
                .map(|s| if s.len() > 50 { format!("{}...", &s[..50]) } else { s.to_string() })
                .unwrap_or_default();
            parts.push(format!("    - {} ({}): {}", sym.name, sym.kind.as_str(), sig_preview));
        }
    }
    
    // Callers (who uses this code)
    if !context.callers.is_empty() {
        parts.push("  Called by:".to_string());
        for caller in &context.callers {
            parts.push(format!("    - {}", caller));
        }
    }
    
    // Callees (what this code depends on)
    if !context.callees.is_empty() {
        parts.push("  Depends on:".to_string());
        for callee in &context.callees {
            parts.push(format!("    - {}", callee));
        }
    }
    
    // Related files
    if !context.related_files.is_empty() {
        parts.push("  Related files:".to_string());
        for file in &context.related_files {
            parts.push(format!("    - {}", file));
        }
    }
}

fn add_memory_context(parts: &mut Vec<String>, context: &ReviewContext) {
    if let Some(memory) = &context.memory_context {
        if !memory.is_empty() {
            parts.push("\n🧠 Project Memory:".to_string());
            // Limit memory context to prevent overwhelming
            let memory_preview = if memory.len() > 1000 {
                format!("{}...\n(truncated)", &memory[..1000])
            } else {
                memory.clone()
            };
            parts.push(format!("  {}", memory_preview));
        }
    }
}

fn parse_review_result(response: &str) -> Result<PreWriteReviewResult> {
    let json_str = extract_json(response)?;
    let mut result: PreWriteReviewResult = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse review result: {}", e))?;
    
    if result.overall_score > 100 {
        log::warn!("Review score {} exceeds 100, clamping to 100", result.overall_score);
        result.overall_score = 100;
    }
    
    if result.impact_analysis.affected_modules.is_empty() {
        result.impact_analysis.affected_modules.push("unknown".to_string());
    }
    
    Ok(result)
}

fn extract_json(response: &str) -> Result<String> {
    // Try markdown code block
    if let Some(start_idx) = response.find("```json") {
        let rest = &response[start_idx + 7..];
        if let Some(end_idx) = rest.find("```") {
            return Ok(rest[..end_idx].trim().to_string());
        }
    }
    
    // Try plain JSON
    if let Some(start_idx) = response.find('{') {
        let rest = &response[start_idx..];
        let mut brace_count = 0;
        for (i, ch) in rest.chars().enumerate() {
            if ch == '{' {
                brace_count += 1;
            } else if ch == '}' {
                brace_count -= 1;
                if brace_count == 0 {
                    return Ok(rest[..=i].trim().to_string());
                }
            }
        }
    }
    
    // Fallback: allow write to proceed
    log::warn!("No JSON found in review response, using fallback");
    Ok(serde_json::to_string(&PreWriteReviewResult {
        overall_score: 70,
        issues: vec![],
        impact_analysis: ImpactAnalysis::default(),
        suggestions: vec!["Review parsing failed, proceeding with caution".to_string()],
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_json_from_markdown() {
        let response = r#"```json
{"overall_score": 85, "issues": [], "impact_analysis": {}, "suggestions": []}
```"#;
        let json = extract_json(response).unwrap();
        assert!(json.contains("overall_score"));
    }
    
    #[test]
    fn test_extract_json_fallback() {
        let response = "No JSON here";
        let json = extract_json(response).unwrap();
        let result: PreWriteReviewResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.overall_score, 70);
    }
    
    #[test]
    fn test_build_prompt_with_context() {
        let input = PreWriteReviewInput {
            tool_name: "write".to_string(),
            file_path: std::path::PathBuf::from("test.rs"),
            existing_content: None,
            new_content: "fn test() {}".to_string(),
            edit_info: None,
            context: ReviewContext {
                symbols: vec![super::super::SymbolInfo {
                    name: "test".to_string(),
                    kind: super::super::SymbolKind::Function,
                    signature: Some("fn test()".to_string()),
                    doc: None,
                }],
                callers: vec!["main".to_string()],
                callees: vec![],
                lsp_diagnostics: vec![],
                memory_context: None,
                related_files: vec!["lib.rs".to_string()],
            },
        };
        let prompt = build_review_prompt(&input);
        assert!(prompt.contains("CodeGraph Analysis"));
        assert!(prompt.contains("Called by"));
        assert!(prompt.contains("Related files"));
    }
}