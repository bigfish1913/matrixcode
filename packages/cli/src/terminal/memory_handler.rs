//! Memory handling for terminal mode
//!
//! Handles memory retrieval, feedback detection, periodic cleanup, and AI extraction.

use std::path::PathBuf;
use matrixcode_core::{
    AgentEvent, providers::Provider, providers::Message,
    memory::{MemoryStorage, AutoMemory},
};
use crate::constants::{
    MEMORY_MANIFEST_SIZE, MEMORY_INITIAL_SUMMARY_SIZE,
    MEMORY_EXTRACTION_INTERVAL, MEMORY_MIN_ENTRIES_FOR_AI_SELECTION,
};

use matrixcode_core::memory::extract_context_keywords;

/// Handle dynamic memory retrieval based on message content
pub fn update_memory_for_message(
    memory: Option<&AutoMemory>,
    msg: &str,
    fast_provider: Option<&dyn Provider>,
    turn_count: usize,
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    if let Some(mem) = memory {
        let is_first_turn = turn_count == 0;
        let is_simple_msg = matrixcode_core::memory::should_skip_simple_message(msg);
        let has_few_memories = mem.entries.len() < MEMORY_MIN_ENTRIES_FOR_AI_SELECTION;

        if is_first_turn || is_simple_msg || has_few_memories {
            // Use static summary for first turn or simple messages
            let static_summary = mem.generate_prompt_summary(MEMORY_INITIAL_SUMMARY_SIZE);
            if !static_summary.is_empty() {
                agent.update_memory_summary(Some(static_summary));
            }
        } else if let Some(fp) = fast_provider {
            // Use AI selection for complex queries
            let manifest = mem.generate_manifest(MEMORY_MANIFEST_SIZE);
            if !manifest.is_empty() {
                // Note: This needs async, so we handle it differently in the agent task
                // For now, fall back to keyword-based retrieval
                let keywords = extract_context_keywords(msg);
                let contextual_summary = mem.generate_contextual_summary_with_keywords(&keywords, 10);
                if !contextual_summary.is_empty() {
                    agent.update_memory_summary(Some(contextual_summary));
                }
            }
        } else {
            // Fallback to keyword-based retrieval
            let keywords = extract_context_keywords(msg);
            let contextual_summary = mem.generate_contextual_summary_with_keywords(&keywords, 10);
            if !contextual_summary.is_empty() {
                agent.update_memory_summary(Some(contextual_summary));
            }
        }
    }
}

/// Async memory retrieval with AI selection
pub async fn ai_select_memory(
    memory: &AutoMemory,
    msg: &str,
    fast_provider: &dyn Provider,
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    let manifest = memory.generate_manifest(MEMORY_MANIFEST_SIZE);
    if !manifest.is_empty() {
        let selected_indices = matrixcode_core::memory::ai_select_memories(
            msg,
            &manifest,
            fast_provider,
        ).await;

        let selected_entries = memory.get_entries_by_indices(&selected_indices);
        let contextual_summary = if selected_entries.is_empty() {
            memory.generate_prompt_summary(5)
        } else {
            let mut summary = String::from("【相关记忆】\n\n");
            for entry in selected_entries.iter().take(5) {
                summary.push_str(&format!("{} {}\n", entry.category.icon(), entry.content));
            }
            summary
        };

        if !contextual_summary.is_empty() {
            agent.update_memory_summary(Some(contextual_summary));

            if !selected_indices.is_empty() {
                let _ = event_tx.send(AgentEvent::with_data(
                    matrixcode_core::EventType::MemoryLoaded,
                    matrixcode_core::EventData::Memory {
                        summary: format!("AI 选择了 {} 条相关记忆", selected_indices.len()),
                        entries_count: selected_indices.len(),
                    },
                )).await;
            }
        }
    }
}

/// Handle memory feedback detection
pub async fn handle_feedback(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    memory_storage: &mut Option<MemoryStorage>,
    msg: &str,
) {
    if let Some(ms) = memory_storage {
        let feedback_results = matrixcode_core::memory::detect_feedback_patterns(msg);
        if !feedback_results.is_empty()
            && let Ok(mut mem) = ms.load_combined() {
            let feedback_count = feedback_results.len();
            for feedback in feedback_results {
                matrixcode_core::memory::apply_feedback_to_memory(&mut mem, &feedback);
            }
            if mem.entries.iter().any(|e| e.tags.contains(&"project".to_string())) {
                if let Err(e) = ms.save_project(&mem) {
                    log::warn!("Failed to save project memory: {}", e);
                }
            } else {
                if let Err(e) = ms.save_global(&mem) {
                    log::warn!("Failed to save global memory: {}", e);
                }
            }
            let _ = event_tx.send(AgentEvent::progress(
                format!("🧠 Learned from feedback: {} corrections", feedback_count),
                None,
            )).await;
        }
    }
}

/// Handle periodic memory cleanup
pub async fn periodic_cleanup(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    memory_storage: &mut Option<MemoryStorage>,
) {
    if let Some(ms) = memory_storage
        && let Ok(mut mem) = ms.load_combined() {
        mem.apply_time_decay();
        let merged = mem.smart_merge();
        mem.prune();
        if let Err(e) = ms.save_global(&mem) {
            log::warn!("Failed to save memory after maintenance: {}", e);
        }
        if merged > 0 {
            let _ = event_tx.send(AgentEvent::progress(
                format!("🧠 合并了 {} 条相似记忆", merged),
                None,
            )).await;
        }
    }
}

/// Check if memory extraction should run this turn
pub fn should_extract_memory(turn_count: usize, has_fast_provider: bool) -> bool {
    turn_count.is_multiple_of(MEMORY_EXTRACTION_INTERVAL) && has_fast_provider
}

/// Spawn background task for AI memory extraction
pub fn spawn_extraction_task(
    event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    project_path: Option<PathBuf>,
    fast_model: Option<String>,
    last_message: &Message,
) {
    let text = match &last_message.content {
        matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
        matrixcode_core::providers::MessageContent::Blocks(blocks) => {
            blocks.iter().filter_map(|b| match b {
                matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            }).collect::<Vec<_>>().join("\n")
        }
    };

    if text.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let bg_ms = MemoryStorage::new(project_path.as_deref()).ok();
        if bg_ms.is_none() {
            return;
        }
        let mut bg_ms = bg_ms.unwrap();

        let project_path_str = project_path.as_ref().map(|p: &PathBuf| p.to_string_lossy().to_string());
        let detected = if let Some(model) = fast_model {
            matrixcode_core::debug::debug_log().log(
                "memory_extract",
                &format!("Background: extracting with model={}", model)
            );
            let extractor = matrixcode_core::memory::AiMemoryExtractor::new_minimal(model);
            matrixcode_core::memory::detect_memories_smart(
                &text, None, project_path_str.as_deref(), Some(&extractor)
            ).await
        } else {
            Vec::new()
        };

        if !detected.is_empty() {
            let detected_count = detected.len();
            for entry in detected {
                let is_global_category = matches!(
                    entry.category,
                    matrixcode_core::memory::MemoryCategory::Preference
                        | matrixcode_core::memory::MemoryCategory::UserIntentPattern
                        | matrixcode_core::memory::MemoryCategory::TaskPattern
                );
                let is_project = !is_global_category
                    && (entry.tags.contains(&"project".to_string())
                        || entry.project_path.is_some()
                        || project_path.is_some());

                if let Err(e) = bg_ms.add_entry(entry, is_project) {
                    log::warn!("Failed to add memory entry: {}", e);
                }
            }
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::MemoryDetected,
                matrixcode_core::EventData::Memory {
                    summary: format!("检测到 {} 条记忆", detected_count),
                    entries_count: detected_count,
                },
            )).await;
        }
    });
}

/// Load memory storage and return combined memory
pub fn load_memory(project_path: Option<&std::path::Path>) -> (Option<MemoryStorage>, Option<AutoMemory>) {
    let storage = MemoryStorage::new(project_path).ok();
    let memory = storage.as_ref().and_then(|ms| ms.load_combined().ok());
    (storage, memory)
}