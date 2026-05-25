use matrixcode_core::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Role, StopReason, Usage,
};
use matrixcode_core::tools::ToolDefinition;
use serde_json::json;

#[test]
fn test_message_creation() {
    let msg = Message {
        role: Role::User,
        content: MessageContent::Text("hello".to_string()),
    };
    assert_eq!(msg.role, Role::User);
    match msg.content {
        MessageContent::Text(ref t) => assert_eq!(t, "hello"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn test_content_block_text() {
    let block = ContentBlock::Text {
        text: "test".to_string(),
    };
    match block {
        ContentBlock::Text { text } => assert_eq!(text, "test"),
        _ => panic!("expected text block"),
    }
}

#[test]
fn test_content_block_tool_use() {
    let block = ContentBlock::ToolUse {
        id: "id-1".to_string(),
        name: "read".to_string(),
        input: json!({"path": "test.rs"}),
    };
    match block {
        ContentBlock::ToolUse { id, name, input } => {
            assert_eq!(id, "id-1");
            assert_eq!(name, "read");
            assert_eq!(input["path"], "test.rs");
        }
        _ => panic!("expected tool_use block"),
    }
}

#[test]
fn test_content_block_tool_result() {
    let block = ContentBlock::ToolResult {
        tool_use_id: "id-1".to_string(),
        content: "result text".to_string(),
    };
    match block {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
        } => {
            assert_eq!(tool_use_id, "id-1");
            assert_eq!(content, "result text");
        }
        _ => panic!("expected tool_result block"),
    }
}

#[test]
fn test_chat_request_creation() {
    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("hello".to_string()),
        }],
        tools: vec![ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            parameters: json!({}),
            ..Default::default()
        }],
        system: Some("You are helpful".to_string()),
        think: false,
        max_tokens: 4096,
        server_tools: vec![],
        enable_caching: false,
    };

    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.system.unwrap(), "You are helpful");
}

#[test]
fn test_chat_response_creation() {
    let response = ChatResponse {
        content: vec![ContentBlock::Text {
            text: "Hello!".to_string(),
        }],
        stop_reason: StopReason::EndTurn,
        usage: Usage::default(),
    };

    assert_eq!(response.content.len(), 1);
    assert_eq!(response.stop_reason, StopReason::EndTurn);
}

#[test]
fn test_stop_reason_variants() {
    assert_eq!(StopReason::EndTurn, StopReason::EndTurn);
    assert_eq!(StopReason::ToolUse, StopReason::ToolUse);
    assert_eq!(StopReason::MaxTokens, StopReason::MaxTokens);
    assert_ne!(StopReason::EndTurn, StopReason::ToolUse);
}

#[test]
fn test_role_variants() {
    assert_eq!(Role::System, Role::System);
    assert_eq!(Role::User, Role::User);
    assert_eq!(Role::Assistant, Role::Assistant);
    assert_eq!(Role::Tool, Role::Tool);
    assert_ne!(Role::User, Role::Assistant);
}

#[test]
fn test_message_content_blocks() {
    let content = MessageContent::Blocks(vec![
        ContentBlock::Text {
            text: "thinking".to_string(),
        },
        ContentBlock::ToolUse {
            id: "1".to_string(),
            name: "read".to_string(),
            input: json!({"path": "a.rs"}),
        },
    ]);

    match content {
        MessageContent::Blocks(blocks) => assert_eq!(blocks.len(), 2),
        _ => panic!("expected blocks"),
    }
}
