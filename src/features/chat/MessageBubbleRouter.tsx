import { Message } from '@/models/chat';
import React from 'react';
import ContentBubble from './ContentBubble';
import ToolCallResultBubble from './ToolCallResultBubble';

interface MessageBubbleRouterProps {
  message: Message;
  nextMessages?: Message[]; // Tool result messages following this message
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
  nextMessages = [],
}) => {
  const hasToolCalls =
    message.tool_calls &&
    Array.isArray(message.tool_calls) &&
    message.tool_calls.length > 0 &&
    message.tool_calls.every((tc) => tc && tc.function && tc.function.name);

  const hasText = !!(message.content && message.content.length > 0);

  // If the message has tool calls, use the integrated bubble
  if (hasToolCalls) {
    return (
      <>
        {hasText && <ContentBubble message={message} />}
        {message.tool_calls!.map((toolCall) => {
          // Find the matching tool result by tool_call_id
          const toolResult = nextMessages.find(
            (m) => m.role === 'tool' && m.tool_call_id === toolCall.id,
          );

          return (
            <ToolCallResultBubble
              key={toolCall.id}
              toolCall={toolCall}
              toolResult={toolResult}
              isLoading={!toolResult}
            />
          );
        })}
      </>
    );
  }

  // Standalone tool messages are already rendered with their parent assistant message
  // So we return null to avoid duplicate rendering
  if (message.role === 'tool') {
    return null;
  }

  return <ContentBubble message={message} />;
};

export default MessageBubbleRouter;
