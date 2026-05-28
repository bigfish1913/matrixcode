//! /compact command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};
use crate::compress::{CompressionConfig, CompressionStrategy, estimate_total_tokens, compress_messages};

pub struct CompactCommand;

impl Command for CompactCommand {
    fn name(&self) -> &'static str {
        "compact"
    }

    fn aliases(&self) -> &[&'static str] {
        &["compress"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Compact conversation history to save tokens")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let original_tokens = estimate_total_tokens(ctx.agent.get_messages());
            if original_tokens > 100 {
                let _ = ctx.event_tx.send(crate::AgentEvent::with_data(
                    crate::EventType::CompressionTriggered,
                    crate::EventData::Progress {
                        message: format!("Compressing {} tokens...", original_tokens),
                        percentage: None,
                    },
                )).await;

                match compress_messages(
                    ctx.agent.get_messages(),
                    CompressionStrategy::SlidingWindow,
                    &CompressionConfig::default(),
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = estimate_total_tokens(&compressed);
                        ctx.agent.set_messages(compressed);
                        let ratio = compressed_tokens as f32 / original_tokens as f32;

                        let _ = ctx.event_tx.send(crate::AgentEvent::with_data(
                            crate::EventType::CompressionCompleted,
                            crate::EventData::Compression {
                                original_tokens: original_tokens as u64,
                                compressed_tokens: compressed_tokens as u64,
                                ratio,
                            },
                        )).await;
                    }
                    Err(e) => {
                        let _ = ctx.event_tx.send(crate::AgentEvent::error(
                            format!("Compression failed: {}", e),
                            None,
                            None,
                        )).await;
                    }
                }
            } else {
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                    "Context too small, no need to compress".to_string(),
                    None,
                )).await;
            }
            false
        })
    }
}