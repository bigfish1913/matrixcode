//! /cron command

use std::time::{Duration, Instant};

use matrixcode_core::cancel::CancellationToken;

use crate::app::CronTask;
use crate::commands::{Command, CommandContext};
use crate::utils::truncate;

pub struct CronCommand;

impl Command for CronCommand {
    fn name(&self) -> &'static str {
        "cron"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Manage scheduled tasks")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
        if args.is_empty() {
            ctx.push_system(
                "/cron add <message> <minutes> - Add cron task\n/cron list - List tasks\n/cron remove <id> - Remove task\n/cron clear - Clear all".into(),
            );
        } else if args[0] == "list" {
            if ctx.app.cron_tasks.is_empty() {
                ctx.push_system("No cron tasks".into());
            } else {
                let list: Vec<String> = ctx
                    .app
                    .cron_tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "#{}: '{}' every {}min",
                            t.id,
                            truncate(&t.message, 20),
                            t.minute_interval
                        )
                    })
                    .collect();
                ctx.push_system(format!("📋 Cron tasks:\n{}", list.join("\n")));
            }
        } else if args[0] == "remove" || args[0] == "rm" {
            let id: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            if let Some(pos) = ctx.app.cron_tasks.iter().position(|t| t.id == id) {
                let task = &ctx.app.cron_tasks[pos];
                task.cancel_token.cancel();
                ctx.app.cron_tasks.remove(pos);
                ctx.push_system(format!("✓ Cron task #{} removed", id));
            } else {
                ctx.push_system(format!("Cron task #{} not found", id));
            }
        } else if args[0] == "clear" {
            for task in &ctx.app.cron_tasks {
                task.cancel_token.cancel();
            }
            let count = ctx.app.cron_tasks.len();
            ctx.app.cron_tasks.clear();
            ctx.push_system(format!("✓ {} cron tasks cleared", count));
        } else if args[0] == "add" {
            // /cron add "message" 5
            if args.len() < 3 {
                ctx.push_system("Usage: /cron add <message> <minutes>".into());
            } else {
                let message = args[1].to_string();
                let minute_interval: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

                let id = ctx.app.cron_tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                let cancel_token = CancellationToken::new();

                let task = CronTask {
                    id,
                    message: message.clone(),
                    minute_interval,
                    next_run: Instant::now() + Duration::from_secs(minute_interval * 60),
                    cancel_token: cancel_token.clone(),
                };

                ctx.app.cron_tasks.push(task);

                // Spawn background task
                let tx = ctx.app.tx.clone();
                let msg = message.clone();
                let interval_secs = minute_interval * 60;
                tokio::spawn(async move {
                    // Initial delay to next_run
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
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
                    "✓ Cron #{} added: '{}' every {}min",
                    id,
                    truncate(&message, 30),
                    minute_interval
                ));
            }
        } else {
            ctx.push_system("Unknown cron command. Use: add, list, remove, clear".into());
        }
        ctx.auto_scroll();
    }
}