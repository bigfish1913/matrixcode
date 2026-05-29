//! /loop command

use std::time::Duration;

use matrixcode_core::cancel::CancellationToken;

use crate::app::LoopTask;
use crate::commands::{Command, CommandContext};
use crate::utils::truncate;

pub struct LoopCommand;

impl Command for LoopCommand {
    fn name(&self) -> &'static str {
        "loop"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Start/stop loop task")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
        if args.is_empty() {
            ctx.push_system(
                "/loop <message> [interval] [count] - Start loop\n/loop stop - Stop loop\n/loop status - Show status".into(),
            );
        } else if args[0] == "stop" {
            // Take ownership to avoid borrow conflict
            let task = ctx.app.loop_task.take();
            if let Some(ref task) = task {
                task.cancel_token.cancel();
                ctx.push_system(format!("✓ Loop stopped (executed {} times)", task.count));
                ctx.app.loop_task = None;
            } else {
                ctx.push_system("No active loop".into());
            }
        } else if args[0] == "status" {
            if let Some(ref task) = ctx.app.loop_task {
                ctx.push_system(format!(
                    "🔄 Loop active: '{}' every {}s, count {}{}",
                    truncate(&task.message, 30),
                    task.interval_secs,
                    task.count,
                    task.max_count
                        .map(|m| format!(" (max {})", m))
                        .unwrap_or_default()
                ));
            } else {
                ctx.push_system("No active loop".into());
            }
        } else {
            // Start new loop: /loop "message" [interval] [max_count]
            if ctx.app.loop_task.is_some() {
                ctx.push_system("⚠️ Loop already active. Use /loop stop first".into());
            } else {
                let message = args[0].to_string();
                let interval_secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
                let max_count: Option<u64> = args.get(2).and_then(|s| s.parse().ok());

                let cancel_token = CancellationToken::new();
                ctx.app.loop_task = Some(LoopTask {
                    message: message.clone(),
                    interval_secs,
                    count: 0,
                    max_count,
                    cancel_token: cancel_token.clone(),
                });

                // Spawn background task
                let tx = ctx.app.tx.clone();
                let msg = message.clone();
                tokio::spawn(async move {
                    loop {
                        if cancel_token.is_cancelled() {
                            break;
                        }
                        // Send message
                        tx.try_send(msg.clone()).ok();
                        // Wait interval
                        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                    }
                });

                ctx.push_system(format!(
                    "🔄 Loop started: '{}' every {}s{}",
                    truncate(&message, 30),
                    interval_secs,
                    max_count
                        .map(|m| format!(" (max {})", m))
                        .unwrap_or_default()
                ));
            }
        }
        ctx.auto_scroll();
    }
}
