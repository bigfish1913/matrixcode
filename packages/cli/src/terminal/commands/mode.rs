//! /mode command handler

use matrixcode_core::approval::ApproveMode;

/// Handle /mode command
pub fn handle_mode(msg: &str, agent: &mut matrixcode_core::agent::Agent) -> bool {
    if let Some(mode_str) = msg.strip_prefix("/mode:") {
        let new_mode = match mode_str {
            "ask" => ApproveMode::Ask,
            "auto" => ApproveMode::Auto,
            "strict" => ApproveMode::Strict,
            _ => return false,
        };
        agent.set_approve_mode(new_mode);
        return true;
    }
    false
}