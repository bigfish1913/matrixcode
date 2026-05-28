//! Skill activation handler

use matrixcode_core::{AgentEvent, skills::Skill};

/// Check if message is a skill activation and return transformed message
pub fn activate_skill(msg: &str, skills: &[Skill]) -> Option<(String, String)> {
    if !msg.starts_with('/') {
        return None;
    }
    
    // Skip other commands
    if msg.starts_with("/skills")
        || msg.starts_with("/workflow")
        || msg.starts_with("/compact") || msg.starts_with("/compress")
        || msg.starts_with("/help") || msg.starts_with("/init")
        || msg.starts_with("/memory") || msg.starts_with("/overview")
        || msg.starts_with("/save") || msg.starts_with("/sessions")
        || msg.starts_with("/resume") || msg.starts_with("/loop")
        || msg.starts_with("/exit") || msg.starts_with("/quit")
        || msg.starts_with("/clear") || msg.starts_with("/debug")
        || msg.starts_with("/status") || msg.starts_with("/new")
        || msg.starts_with("/load") || msg.starts_with("/mode")
        || msg.starts_with("/model") || msg.starts_with("/retry")
        || msg.starts_with("/history") || msg.starts_with("/cron")
        || msg.starts_with("/config") || msg.starts_with("/tools")
        || msg.starts_with("/system")
        || msg == "/" {
        return None;
    }
    
    let skill_name = msg.trim_start_matches('/');
    if let Some(skill) = skills.iter().find(|s| s.name == skill_name) {
        let files = matrixcode_core::skills::list_skill_files(&skill.dir);
        let files_info = if files.len() > 1 {
            format!("\n\n📁 Associated files:\n{}",
                files.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"))
        } else {
            String::new()
        };

        let transformed_msg = format!(
            "使用 skill '{}' 来处理当前任务。\n\n---\n{}\n---\n{}\n\n请按照上述 skill 指导开始执行。",
            skill.name,
            skill.body,
            files_info
        );
        
        let notification = format!("🎯 Activating skill: {}", skill.name);
        
        Some((transformed_msg, notification))
    } else {
        None
    }
}