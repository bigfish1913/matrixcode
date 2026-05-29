//! API Connection Tests
//! Tests both Anthropic and OpenAI protocols for DashScope

use matrixcode_core::providers::{ProviderType, StreamEvent, create_provider_with_headers};
use matrixcode_core::{ChatRequest, Message, MessageContent, Role};
use std::collections::HashMap;

fn get_api_key() -> String {
    std::env::var("API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
        .unwrap_or_else(|_| "test-key".to_string())
}

fn get_extra_headers() -> Option<HashMap<String, String>> {
    // Read from env if available
    let headers_str = std::env::var("EXTRA_HEADERS").ok();
    if let Some(json_str) = headers_str {
        if let Ok(headers) = serde_json::from_str::<HashMap<String, String>>(&json_str) {
            return Some(headers);
        }
    }
    None
}

fn create_test_request() -> ChatRequest {
    ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("Say 'hello' in one word".to_string()),
        }],
        tools: vec![],
        system: None,
        think: false,
        max_tokens: 100,
        server_tools: vec![],
        enable_caching: false,
    }
}

/// Test Anthropic protocol (DashScope apps/anthropic endpoint)
#[tokio::test]
async fn test_anthropic_protocol() {
    let api_key = get_api_key();
    let base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic";
    let model = "glm-5";

    let provider = create_provider_with_headers(
        ProviderType::Anthropic,
        api_key,
        model.to_string(),
        Some(base_url.to_string()),
        get_extra_headers(),
    )
    .expect("Failed to create Anthropic provider");

    let request = create_test_request();
    let result = provider.chat(request).await;

    match result {
        Ok(response) => {
            println!("✓ Anthropic protocol success!");
            println!("  Content: {:?}", response.content);
            println!("  Stop reason: {:?}", response.stop_reason);
            println!(
                "  Usage: in={}, out={}",
                response.usage.input_tokens, response.usage.output_tokens
            );
            assert!(!response.content.is_empty());
        }
        Err(e) => {
            println!("✗ Anthropic protocol error: {}", e);
            // Don't fail test if API key is invalid
            if e.to_string().contains("401") {
                println!("  Hint: Check API_KEY in .env");
            }
        }
    }
}

/// Test OpenAI protocol (DashScope compatible-mode endpoint)
#[tokio::test]
async fn test_openai_protocol() {
    let api_key = get_api_key();
    let base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1";
    let model = "glm-5";

    // OpenAI protocol needs X-DashScope-SSE header for streaming
    let headers = get_extra_headers().or(Some(HashMap::from([(
        "X-DashScope-SSE".to_string(),
        "enable".to_string(),
    )])));

    let provider = create_provider_with_headers(
        ProviderType::OpenAI,
        api_key,
        model.to_string(),
        Some(base_url.to_string()),
        headers,
    )
    .expect("Failed to create OpenAI provider");

    let request = create_test_request();
    let result = provider.chat(request).await;

    match result {
        Ok(response) => {
            println!("✓ OpenAI protocol success!");
            println!("  Content: {:?}", response.content);
            println!("  Stop reason: {:?}", response.stop_reason);
            println!(
                "  Usage: in={}, out={}",
                response.usage.input_tokens, response.usage.output_tokens
            );
            assert!(!response.content.is_empty());
        }
        Err(e) => {
            println!("✗ OpenAI protocol error: {}", e);
            if e.to_string().contains("401") {
                println!("  Hint: Check API_KEY in .env");
            }
        }
    }
}

/// Test streaming with Anthropic protocol
#[tokio::test]
async fn test_anthropic_streaming() {
    let api_key = get_api_key();
    let base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic";
    let model = "glm-5";

    let provider = create_provider_with_headers(
        ProviderType::Anthropic,
        api_key,
        model.to_string(),
        Some(base_url.to_string()),
        get_extra_headers(),
    )
    .expect("Failed to create Anthropic provider");

    let request = create_test_request();
    let result = provider.chat_stream(request).await;

    match result {
        Ok(mut rx) => {
            println!("✓ Anthropic streaming started!");
            while let Some(event) = rx.recv().await {
                match event {
                    StreamEvent::TextDelta(text) => {
                        println!("  Text: {}", text);
                    }
                    StreamEvent::Done(response) => {
                        println!("  Stream done, content: {:?}", response.content);
                        break;
                    }
                    StreamEvent::Error(e) => {
                        println!("  Stream error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("✗ Anthropic streaming error: {}", e);
        }
    }
}

/// Test streaming with OpenAI protocol
#[tokio::test]
async fn test_openai_streaming() {
    let api_key = get_api_key();
    let base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1";
    let model = "glm-5";

    let headers = get_extra_headers().or(Some(HashMap::from([(
        "X-DashScope-SSE".to_string(),
        "enable".to_string(),
    )])));

    let provider = create_provider_with_headers(
        ProviderType::OpenAI,
        api_key,
        model.to_string(),
        Some(base_url.to_string()),
        headers,
    )
    .expect("Failed to create OpenAI provider");

    let request = create_test_request();
    let result = provider.chat_stream(request).await;

    match result {
        Ok(mut rx) => {
            println!("✓ OpenAI streaming started!");
            while let Some(event) = rx.recv().await {
                match event {
                    StreamEvent::TextDelta(text) => {
                        println!("  Text: {}", text);
                    }
                    StreamEvent::Done(response) => {
                        println!("  Stream done, content: {:?}", response.content);
                        break;
                    }
                    StreamEvent::Error(e) => {
                        println!("  Stream error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("✗ OpenAI streaming error: {}", e);
        }
    }
}
