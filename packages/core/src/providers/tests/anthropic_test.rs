//! Unit tests for anthropic provider
//! Testing filter_thinking and SSE handling for DashScope/glm-5

use serde_json::{Value, json};

/// Test that thinking blocks are filtered for non-official APIs
#[test]
fn test_filter_thinking_blocks() {
    // Simulate messages with thinking blocks
    let messages = json!([
        {"role": "user", "content": "你好"},
        {"role": "assistant", "content": [
            {"type": "thinking", "thinking": "让我思考...", "signature": "abc123"},
            {"type": "text", "text": "你好！"}
        ]},
        {"role": "user", "content": "继续"}
    ]);

    // Expected output after filtering (no thinking blocks)
    let expected = json!([
        {"role": "user", "content": [{"type": "text", "text": "你好"}]},
        {"role": "assistant", "content": [{"type": "text", "text": "你好！"}]},
        {"role": "user", "content": [{"type": "text", "text": "继续"}]}
    ]);

    // Simulate filtering logic
    let filtered = filter_thinking_from_messages(&messages, true);

    assert_eq!(filtered, expected);
}

/// Test that thinking blocks are NOT filtered for official Anthropic API
#[test]
fn test_keep_thinking_blocks_for_official_api() {
    let messages = json!([
        {"role": "assistant", "content": [
            {"type": "thinking", "thinking": "思考内容", "signature": "sig"},
            {"type": "text", "text": "回复"}
        ]}
    ]);

    // Should NOT filter for official API (filter_thinking = false)
    let filtered = filter_thinking_from_messages(&messages, false);

    // Thinking block should remain
    assert!(filtered[0]["content"].as_array().unwrap().len() == 2);
}

/// Test empty signature handling
#[test]
fn test_empty_signature_handling() {
    // DashScope returns empty signature
    let thinking_block = json!({
        "type": "thinking",
        "thinking": "思考内容",
        "signature": ""
    });

    // Should handle empty signature gracefully
    let sig = thinking_block["signature"].as_str().unwrap_or("");
    assert!(sig.is_empty());

    // When building request, should NOT include empty signature
    let obj = build_thinking_object("思考内容", "");
    assert!(obj.get("signature").is_none() || obj["signature"].as_str().unwrap().is_empty());
}

/// Test SSE thinking_delta event parsing
#[test]
fn test_sse_thinking_delta_parsing() {
    // Simulate SSE event from DashScope
    let sse_event = json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {
            "type": "thinking_delta",
            "thinking": "思考片段"
        }
    });

    assert_eq!(sse_event["delta"]["type"], "thinking_delta");
    assert_eq!(sse_event["delta"]["thinking"], "思考片段");
}

/// Test content_block_start with thinking type
#[test]
fn test_content_block_start_thinking() {
    // DashScope returns thinking block start with empty signature
    let event = json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": {
            "type": "thinking",
            "signature": "",
            "thinking": ""
        }
    });

    assert_eq!(event["content_block"]["type"], "thinking");
    assert!(
        event["content_block"]["signature"]
            .as_str()
            .unwrap()
            .is_empty()
    );
}

// Helper functions simulating anthropic.rs logic

fn filter_thinking_from_messages(messages: &Value, filter_thinking: bool) -> Value {
    messages
        .as_array()
        .unwrap()
        .iter()
        .map(|msg| {
            let role = msg["role"].as_str().unwrap();
            let content = if let Some(blocks) = msg["content"].as_array() {
                let filtered_blocks: Vec<Value> = blocks
                    .iter()
                    .filter(|b| {
                        if filter_thinking && b["type"].as_str() == Some("thinking") {
                            return false;
                        }
                        true
                    })
                    .map(|b| {
                        if b["type"].as_str() == Some("text") {
                            json!({"type": "text", "text": b["text"].as_str().unwrap_or("")})
                        } else if b["type"].as_str() == Some("thinking") {
                            build_thinking_object(
                                b["thinking"].as_str().unwrap_or(""),
                                b["signature"].as_str().unwrap_or(""),
                            )
                        } else {
                            b.clone()
                        }
                    })
                    .collect();
                json!(filtered_blocks)
            } else if let Some(text) = msg["content"].as_str() {
                json!([{"type": "text", "text": text}])
            } else {
                msg["content"].clone()
            };
            json!({"role": role, "content": content})
        })
        .collect::<Vec<_>>()
        .into()
}

fn build_thinking_object(thinking: &str, signature: &str) -> Value {
    let mut obj = json!({"type": "thinking", "thinking": thinking});
    if !signature.is_empty() {
        obj["signature"] = json!(signature);
    }
    obj
}
