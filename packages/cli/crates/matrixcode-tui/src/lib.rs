//! MatrixCode Terminal UI - Rendering and Animation
//!
//! 这个 crate 处理所有终端渲染：
//! - Spinner/进度指示
//! - Markdown 渲染
//! - Tool Use 可视化
//! - 颜色/样式
//!
//! 它接收 Core 的 AgentEvent，负责渲染到终端。

use matrixcode_core::{AgentEvent, EventData, EventType};

/// TUI 版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Terminal UI 处理器
///
/// 接收 AgentEvent 并渲染到终端
pub struct TerminalUI {
    // TODO: 迁移后添加字段
    // spinner: Spinner,
    // markdown_renderer: MarkdownRenderer,
}

impl TerminalUI {
    /// 创建新的 Terminal UI
    pub fn new() -> Self {
        Self {}
    }

    /// 处理事件并渲染
    pub fn handle_event(&mut self, event: &AgentEvent) {
        match event.event_type {
            EventType::TextStart => {
                // 停止 spinner，准备显示文本
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = &event.data {
                    // 直接打印文本
                    print!("{}", delta);
                }
            }
            EventType::TextEnd => {
                // 文本结束，可以换行
                println!();
            }
            EventType::ThinkingStart => {
                // 显示 thinking 开始
                eprintln!("[Thinking...]");
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = &event.data {
                    eprint!("{}", delta);
                }
            }
            EventType::ThinkingEnd => {
                eprintln!();
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { id, name, .. }) = &event.data {
                    eprintln!("[Tool: {} (id: {})]", name, id);
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, is_error, .. }) = &event.data {
                    if *is_error {
                        eprintln!("[Error: {}]", content);
                    } else {
                        eprintln!("[Result: {}]", truncate(&content, 100));
                    }
                }
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = &event.data {
                    eprintln!("\n❌ Error: {}", message);
                }
            }
            EventType::SessionStarted => {
                eprintln!("--- Session Started ---");
            }
            EventType::SessionEnded => {
                eprintln!("--- Session Ended ---");
            }
            EventType::Usage => {
                if let Some(EventData::Usage { input_tokens, output_tokens, .. }) = &event.data {
                    eprintln!("\n📊 Tokens: {} in, {} out", input_tokens, output_tokens);
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, percentage }) = &event.data {
                    if let Some(p) = percentage {
                        eprintln!("[{}%] {}", p, message);
                    } else {
                        eprintln!("⏳ {}", message);
                    }
                }
            }
            _ => {
                // 其他事件类型
            }
        }
    }

    /// 处理 JSON 字符串（从 Core 输出）
    pub fn handle_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let event = AgentEvent::from_json(json)?;
        self.handle_event(&event);
        Ok(())
    }

    /// 处理事件列表
    pub fn handle_events(&mut self, events: &[AgentEvent]) {
        for event in events {
            self.handle_event(event);
        }
    }
}

impl Default for TerminalUI {
    fn default() -> Self {
        Self::new()
    }
}

/// 截断字符串
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_text_event() {
        let mut ui = TerminalUI::new();
        let event = AgentEvent::text_delta("Hello".to_string());
        ui.handle_event(&event);
    }

    #[test]
    fn test_handle_json() {
        let mut ui = TerminalUI::new();
        let event = AgentEvent::session_started();
        let json = event.to_json().unwrap();
        ui.handle_json(&json).unwrap();
    }
}
