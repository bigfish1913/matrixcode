use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::truncate::truncate_string_in_place;

pub struct BashTool;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT: usize = 30_000;

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "在当前工作目录执行 shell 命令，返回合并的 stdout + stderr。\
                 用于构建、测试、git、包管理器等操作。命令通过 `sh -c` 执行并有超时限制。"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "要执行的 shell 命令"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "最大运行时间（毫秒，默认 120000，最大 600000）"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // Create spinner immediately at the start to fill the gap before actual operation
        // let mut spinner = ToolSpinner::new("preparing command");

        let command = params["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'command'"))?;

        if let Some(reason) = refuse_reason(command) {
            // spinner.finish_error("refused");
            anyhow::bail!("refused: {}", reason);
        }

        let timeout_ms = params["timeout_ms"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        // Update spinner message for the actual command execution
        // spinner.set_message(&format!("running: {}", truncate_command(command, 50)));

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command).kill_on_drop(true);

        let fut = cmd.output();
        let output = match tokio::time::timeout(Duration::from_millis(timeout_ms), fut).await {
            Ok(result) => result?,
            Err(_) => {
                // spinner.finish_error("timed out");
                anyhow::bail!("command timed out after {} ms", timeout_ms);
            }
        };

        let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            if !stdout.is_empty() {
                stdout.push('\n');
            }
            stdout.push_str(&stderr);
        }

        let stdout = truncate_output(stdout);

        let code = output.status.code().unwrap_or(-1);
        if !output.status.success() {
            // spinner.finish_error(&format!("exit {}", code));
            return Ok(format!("[exit {}]\n{}", code, stdout));
        }

        // spinner.finish_success("done");
        Ok(stdout)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Dangerous
    }
}

/// Very conservative reject-list covering clearly catastrophic commands.
/// The goal is not a sandbox — it's a last-line guard against obvious
/// accidents like `rm -rf /`. Anything subtle is the caller's responsibility.
fn refuse_reason(cmd: &str) -> Option<&'static str> {
    let norm: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");

    const BANNED_EXACT_PREFIXES: &[&str] = &[
        "rm -rf /",
        "rm -rf /*",
        "rm -rf ~",
        "rm -rf $HOME",
        "rm -rf --no-preserve-root /",
        ":(){:|:&};:",
        "dd if=/dev/zero of=/dev/",
        "mkfs",
        "shutdown",
        "reboot",
        "halt",
    ];

    for bad in BANNED_EXACT_PREFIXES {
        if norm.starts_with(bad) {
            return Some("destructive command blocked");
        }
    }
    if norm.contains("rm -rf /") && !norm.contains("rm -rf /tmp") {
        return Some("destructive rm -rf on root paths blocked");
    }
    None
}

fn truncate_output(mut s: String) -> String {
    if s.len() <= MAX_OUTPUT {
        return s;
    }
    truncate_string_in_place(&mut s, MAX_OUTPUT);
    s.push_str(&format!(
        "\n... (truncated, output exceeded {} bytes)",
        MAX_OUTPUT
    ));
    s
}
