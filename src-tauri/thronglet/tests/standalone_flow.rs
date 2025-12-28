use serde_json::json;
use std::sync::Arc;
use thronglet::agent::Agent;
use thronglet::mock::{MockLLMProvider, MockToolProvider};
use thronglet::models::{
    AgentConfig, Content, FunctionCall, LLMResponse, Message, ToolCall, ToolResult,
};

#[tokio::test]
async fn test_recursive_agent_flow() {
    // 1. Setup Mocks
    let mock_llm = Arc::new(MockLLMProvider::new());
    let mock_tools = Arc::new(MockToolProvider::new());

    // Scenario: User asks "Calculate 1+1", Agent calls tool, then answers "2"

    // Step 1: LLM sees "Calculate 1+1", returns Tool Call for calculator
    let tool_response = LLMResponse {
        content: vec![Content::Text {
            text: "Thinking...".to_string(),
        }],
        tool_calls: Some(vec![ToolCall {
            id: "call_1".to_string(),
            function: FunctionCall {
                name: "calculator__add".to_string(),
                arguments: json!({ "a": 1, "b": 1 }).to_string(),
            },
        }]),
        usage: None,
    };
    mock_llm.push_response("Calculate", tool_response);

    // Step 2: LLM sees Tool Result, returns Final Answer "2"
    let final_response = LLMResponse {
        content: vec![Content::Text {
            text: "The answer is 2".to_string(),
        }],
        tool_calls: None,
        usage: None,
    };
    mock_llm.push_response("TOOL_RESULT", final_response);

    // Mock Tool Execution
    mock_tools.register_tool(
        "calculator",
        "add",
        ToolResult {
            tool_call_id: "call_1".to_string(),
            content: vec![Content::Text {
                text: "2".to_string(),
            }],
            is_error: false,
        },
    );

    // 2. Initialize Agent
    let config = AgentConfig {
        name: "TestAgent".to_string(),
        system_prompt: "You are a test agent".to_string(),
        allowed_mcp_servers: vec!["calculator".to_string()],
        initial_history: None,
    };

    let mut agent = Agent::new(config, mock_llm.clone(), mock_tools.clone());

    // 3. Execution (The Standalone Flow)
    let result = agent.input("Calculate 1+1".to_string()).await;

    // 4. Verification
    assert!(result.is_ok());
    let answer = result.unwrap();
    assert_eq!(answer, "The answer is 2");

    // Verify Tool was actually called
    let calls = mock_tools.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "calculator");
    assert_eq!(calls[0].1, "add");

    // Verify History has 4 messages: User, Assistant(ToolCall), ToolResult, Assistant(Answer)
    let history = agent.history();
    assert_eq!(history.len(), 4);

    match &history[1] {
        Message::Assistant { tool_calls, .. } => assert!(tool_calls.is_some()),
        _ => panic!("Expected Assistant message with tool calls"),
    }

    match &history[2] {
        Message::Tool { content, .. } => {
            if let Content::Text { text } = &content[0] {
                assert_eq!(text, "2");
            }
        }
        _ => panic!("Expected Tool message"),
    }
}
