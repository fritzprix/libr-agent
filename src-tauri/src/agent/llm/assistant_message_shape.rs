use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssistantMessageShape {
    pub has_renderable_content: bool,
    pub has_thinking: bool,
    pub has_tool_calls: bool,
}

impl AssistantMessageShape {
    pub fn is_thinking_only_completion(self) -> bool {
        self.has_thinking && !self.has_renderable_content && !self.has_tool_calls
    }
}

fn content_item_is_renderable(content: &MCPContent) -> bool {
    match content {
        MCPContent::Text { text, .. } => !text.trim().is_empty(),
        MCPContent::Thinking { .. } => false,
        _ => true,
    }
}

fn content_item_has_thinking(content: &MCPContent) -> bool {
    match content {
        MCPContent::Thinking { thinking, .. } => !thinking.trim().is_empty(),
        _ => false,
    }
}

pub fn inspect_assistant_message_shape(message: &Message) -> AssistantMessageShape {
    let has_renderable_content = message.content.iter().any(content_item_is_renderable);
    let has_thinking = message
        .thinking
        .as_ref()
        .map(|thinking| !thinking.trim().is_empty())
        .unwrap_or(false)
        || message.content.iter().any(content_item_has_thinking);
    let has_tool_calls = message
        .tool_calls
        .as_ref()
        .map(|tool_calls| !tool_calls.is_empty())
        .unwrap_or(false)
        || message
            .content
            .iter()
            .any(|content| matches!(content, MCPContent::ToolCall { .. }));

    AssistantMessageShape {
        has_renderable_content,
        has_thinking,
        has_tool_calls,
    }
}
