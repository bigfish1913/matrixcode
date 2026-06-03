//! Prompt Runtime System
//!
//! A dynamic prompt management system with:
//! - Section-based composition (static/dynamic)
//! - Cache management with boundary markers
//! - Runtime context injection
//! - Pre-processing hooks for Skills/Workflows triggering
//! - Prompt export for observability
//! - SessionStart hooks for dynamic injection
//!
//! # Migration Status
//!
//! This module provides both:
//! 1. New dynamic system (PromptOrchestrator, PromptBuilder)
//! 2. Legacy compatibility functions (build_system_prompt, etc.)
//!
//! The legacy functions are gradually being migrated to the new system.

pub mod cache;
pub mod constants;
pub mod context;
pub mod dump;
pub mod hooks;
pub mod orchestrator;
pub mod preprocess;
pub mod section;

// Re-export main types
pub use cache::{
    CacheKey, CacheStats, CachedEntry, SectionCache, clear_global_cache, estimate_tokens,
    global_cache,
};
pub use context::{ContextInjector, ProjectType, SystemContext, UserContext};
pub use dump::{
    DumpEntry, DumpFileAnalysis, PromptAnalysis, PromptDumper, analyze_dump_file, read_dump_file,
};
pub use hooks::{
    DiagnosticEntry, DiagnosticsInjection, SessionStartContext, SessionStartHook, TodoReminder,
};
pub use orchestrator::{
    AssembledPrompt, CACHE_BOUNDARY, PromptBuilder, PromptOrchestrator, PromptProfile,
};
pub use preprocess::{PreProcessHook, ProcessResult, SkillPattern, WorkflowTrigger, preprocess, preprocess_with_skills};
pub use section::{
    PromptSection, SectionBuilder, SectionContent,
    // Predefined sections
    red_flags_section, skill_priority_section, skill_rules_section, tool_guidelines_section,
};

// Re-export constants (includes SECTION_*, MEMORY_*, MSG_*)
pub use constants::*;

// =============================================================================
// Legacy Compatibility Layer
// =============================================================================

/// Legacy profile enum - alias to new PromptProfile
pub use orchestrator::PromptProfile as LegacyPromptProfile;

/// Legacy OverviewContext for project overview generation
/// Used by overview.rs to generate MATRIX.md content
#[derive(Debug, Clone)]
pub struct OverviewContext {
    pub project_name: String,
    pub project_type: String,
    pub directory_structure: String,
    pub config_files: Vec<(String, String)>, // (filename, content)
    pub readme: Option<String>,
    pub source_files: Vec<(String, String)>, // (filename, content)
}

impl Default for OverviewContext {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            project_type: String::new(),
            directory_structure: String::new(),
            config_files: Vec::new(),
            readme: None,
            source_files: Vec::new(),
        }
    }
}

/// Build overview prompt from context (legacy compatibility)
/// This function generates the AI prompt for MATRIX.md generation
pub fn build_overview_prompt(context: &OverviewContext) -> String {
    let mut prompt = String::new();

    // Header
    prompt
        .push_str("你是一个项目分析专家，请根据提供的项目信息生成项目概览文档（MATRIX.md）。\n\n");
    prompt.push_str("要求：\n");
    prompt.push_str("- 使用 Markdown 格式\n");
    prompt.push_str("- 简洁明了，突出重点\n");
    prompt.push_str("- 包含项目定位、技术栈、架构要点\n");
    prompt.push_str("- 便于 AI 助手快速理解项目\n");
    prompt.push_str("\n---\n\n");

    // Add project info
    prompt.push_str(&format!("项目名称: {}\n", context.project_name));
    prompt.push_str(&format!("项目类型: {}\n\n", context.project_type));

    // Add directory structure
    prompt.push_str("## 目录结构\n\n");
    prompt.push_str("```\n");
    prompt.push_str(&context.directory_structure);
    prompt.push_str("```\n\n");

    // Add config files
    if !context.config_files.is_empty() {
        prompt.push_str("## 配置文件\n\n");
        for (filename, content) in &context.config_files {
            prompt.push_str(&format!("### {}\n\n", filename));
            prompt.push_str("```\n");
            prompt.push_str(content);
            prompt.push_str("\n```\n\n");
        }
    }

    // Add README
    if let Some(readme) = &context.readme {
        prompt.push_str("## README.md (开头部分)\n\n");
        prompt.push_str(readme);
        prompt.push_str("\n\n");
    }

    // Add key source files
    if !context.source_files.is_empty() {
        prompt.push_str("## 关键源文件\n\n");
        for (filename, content) in &context.source_files {
            prompt.push_str(&format!("### {}\n\n", filename));
            prompt.push_str("```\n");
            prompt.push_str(content);
            prompt.push_str("\n```\n\n");
        }
    }

    prompt.push_str("---\n\n");
    prompt.push_str("请根据以上信息生成 MATRIX.md 文档内容：\n");

    prompt
}

use crate::skills::Skill;
use std::path::PathBuf;

// Helper for joining iterators
fn join_lines(items: &[String]) -> String {
    items.join("\n")
}

/// Build system prompt using the new orchestrator system
pub fn build_system_prompt(
    profile: &PromptProfile,
    skills: &[Skill],
    project_overview: Option<&str>,
    memory_summary: Option<&str>,
) -> String {
    build_system_prompt_with_workflows(
        profile,
        skills,
        project_overview,
        memory_summary,
        None,
        None,
    )
}

/// Build system prompt with workflow support and LSP injection
///
/// # Arguments
/// - `lsp_servers`: Optional LSP server info list for dynamic injection
/// - `lsp_registry`: Optional LSP registry for tool prompt generation
pub fn build_system_prompt_with_workflows(
    profile: &PromptProfile,
    skills: &[Skill],
    project_overview: Option<&str>,
    memory_summary: Option<&str>,
    project_path: Option<&PathBuf>,
    lsp_servers: Option<&[crate::lsp::LspServerInfo]>,
) -> String {
    build_system_prompt_with_workflows_and_lsp(
        profile,
        skills,
        project_overview,
        memory_summary,
        project_path,
        lsp_servers,
        None,
    )
}

/// Build system prompt with workflow support and full LSP integration
///
/// # Arguments
/// - `lsp_servers`: Optional LSP server info list for dynamic injection
/// - `lsp_registry`: Optional LSP registry for tool prompt generation
pub fn build_system_prompt_with_workflows_and_lsp(
    profile: &PromptProfile,
    skills: &[Skill],
    project_overview: Option<&str>,
    memory_summary: Option<&str>,
    project_path: Option<&PathBuf>,
    lsp_servers: Option<&[crate::lsp::LspServerInfo]>,
    lsp_registry: Option<std::sync::Arc<crate::lsp::LspClientRegistry>>,
) -> String {
    // Determine if CodeGraph is available
    let has_codegraph = project_path
        .map(|p| crate::tools::codegraph::should_inject_codegraph_tools(p))
        .unwrap_or(false);

    // Use orchestrator to build prompt
    let mut builder = PromptBuilder::new(
        project_path
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap()),
    )
    .profile(*profile)
    .no_context();

    // Add static sections from constants
    for (name, content) in constants::get_static_sections(has_codegraph) {
        builder = builder.add_static(name, content);
    }

    // Add memory sections
    for (name, content) in constants::get_memory_sections() {
        builder = builder.add_static(name, content);
    }

    // Build orchestrator
    let mut orchestrator = builder.build();

    // Assemble base prompt
    let assembled = orchestrator.assemble();
    let mut parts = vec![assembled.prompt];

    // Add tools prompt (dynamic based on project_path and LSP registry)
    let tools_prompt = crate::tools::generate_tools_prompt_with_path_and_lsp(project_path, lsp_registry);
    parts.push(tools_prompt);

    // Add Skills section if available
    if !skills.is_empty() {
        let skills_lines: Vec<String> = skills
            .iter()
            .map(|s| {
                if let Some(trigger) = &s.trigger {
                    format!(
                        "  - {}: {}\n    触发场景: {}",
                        s.name, s.description, trigger
                    )
                } else {
                    format!("  - {}: {}", s.name, s.description)
                }
            })
            .collect();
        let skills_section = format!(
            "[AVAILABLE SKILLS]\n可用技能（使用 skill 工具调用）：\n\n{}",
            join_lines(&skills_lines)
        );
        parts.push(skills_section);
    }

    // Add Workflows section if project_path available
    if let Some(path) = project_path {
        let registry = crate::workflow::WorkflowRegistry::new(Some(path));
        if !registry.is_empty() {
            let workflows_lines: Vec<String> = registry
                .list()
                .iter()
                .map(|w| {
                    let desc = w.description.as_deref().unwrap_or(&w.name);
                    if w.required_inputs.is_empty() {
                        format!("  - {}: {}", w.id, desc)
                    } else {
                        format!(
                            "  - {}: {} (需要输入: {})",
                            w.id,
                            desc,
                            w.required_inputs.join(", ")
                        )
                    }
                })
                .collect();
            let workflows_section = format!(
                "[AVAILABLE WORKFLOWS]\n可执行的自动化流程（使用 workflow_run 工具调用）：\n\n{}",
                join_lines(&workflows_lines)
            );
            parts.push(workflows_section);
        }
    }

    // Add project overview if provided
    if let Some(overview) = project_overview {
        parts.push(format!("[PROJECT CONTEXT]\n{}", overview));
    }

    // Add memory summary if provided
    if let Some(memory) = memory_summary {
        parts.push(format!(
            "{}\n{}\n\n{}",
            MEMORY_SUMMARY_HEADER, MEMORY_USAGE_INSTRUCTIONS, memory
        ));
    }

    // Add LSP servers section if active (dynamic injection)
    if let Some(servers) = lsp_servers {
        if let Some(lsp_section) = crate::prompt::constants::get_lsp_section(servers) {
            parts.push(lsp_section);
        }
    }

    parts.join("\n\n")
}
