use serde_json::json;
use thronglet::models::{Content, FunctionCall, LLMResponse, Message, ToolCall};

#[test]
fn test_message_serialization_contract() {
    // 1. Verify User Message
    let user_msg = Message::User {
        content: vec![Content::Text {
            text: "Hello".to_string(),
        }],
    };

    let json = serde_json::to_value(&user_msg).unwrap();
    assert_eq!(
        json,
        json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "Hello" }
            ]
        })
    );

    // 2. Verify Tool Call (Model -> Agent)
    let tool_msg = Message::Assistant {
        content: vec![],
        tool_calls: Some(vec![ToolCall {
            id: "call_123".to_string(),
            function: FunctionCall {
                name: "calculator".to_string(),
                arguments: "{\"a\": 1}".to_string(),
            },
        }]),
    };

    let json = serde_json::to_value(&tool_msg).unwrap();
    assert_eq!(
        json,
        json!({
            "role": "assistant",
            "content": [],
            "tool_calls": [
                {
                    "id": "call_123",
                    "function": {
                        "name": "calculator",
                        "arguments": "{\"a\": 1}"
                    }
                }
            ]
        })
    );
}

#[test]
fn test_llm_response_deserialization_contract() {
    // Simulate what Frontend LLMResponder sends back
    let response_json = json!({
        "content": [
            { "type": "text", "text": "The answer is 42." }
        ],
        "tool_calls": null,
        "usage": null
    });

    let response: LLMResponse =
        serde_json::from_value(response_json).expect("Failed to deserialize LLMResponse");

    assert_eq!(response.content.len(), 1);
    match &response.content[0] {
        Content::Text { text } => assert_eq!(text, "The answer is 42."),
        _ => panic!("Unexpected content type"),
    }
    assert!(response.tool_calls.is_none());
}
