//! Bash command execution tool

mod validator;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::truncate::truncate_string_in_place;
use validator::CommandValidator;

pub use validator::ValidationResult;

pub struct BashTool;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT: usize = 30_000;

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "在当前工作目录执行 shell 命令，返回合并的 stdout + stderr。

IMPORTANT: 当有相关专用工具时，不要用此工具运行命令。使用专用工具更好：

| 命令 | 替代工具 |
|-----|---------|
| cat/head/tail | read |
| sed/awk | edit |
| echo > file | write |
| find/ls | glob |
| grep/rg | search |

将此工具保留用于：
- 构建、测试、git、包管理器操作
- 系统命令和终端操作
- 需要 shell 执行的命令

工作目录在命令间持久，但 shell 状态不持久。
命令通过 `sh -c` 执行，有超时限制（默认 120s，最大 600s）。"
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
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let command = params["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'command'"))?;

        let validator = CommandValidator::new();
        let result = validator.validate(command);
        if !result.allowed {
            anyhow::bail!("refused: {}", result.reason.unwrap_or("unknown"));
        }

        let timeout_ms = params["timeout_ms"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command).kill_on_drop(true);

        let fut = cmd.output();
        let output = match tokio::time::timeout(Duration::from_millis(timeout_ms), fut).await {
            Ok(result) => result?,
            Err(_) => anyhow::bail!("command timed out after {} ms", timeout_ms),
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
            return Ok(format!("[exit {}]\n{}", code, stdout));
        }

        Ok(stdout)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Dangerous
    }
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
