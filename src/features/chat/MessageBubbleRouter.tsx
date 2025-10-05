import { Message } from '@/models/chat';
import React from 'react';
import ContentBubble from './ContentBubble';
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';
import { ErrorBubble } from './ErrorBubble';
import { useChatActions } from '@/context/ChatContext';

interface MessageBubbleRouterProps {
  message: Message;
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
}) => {
  const { retryMessage } = useChatActions();

  // Error message routing - highest priority
  if (message.error) {
    return <ErrorBubble message={message} onRetry={retryMessage} />;
  }

  const hasToolCalls =
    message.tool_calls &&
    Array.isArray(message.tool_calls) &&
    message.tool_calls.length > 0 &&
    message.tool_calls.every((tc) => tc && tc.function && tc.function.name);

  const hasText = !!(message.content && message.content.length > 0);

  // If the message contains both text and tool_calls, render both.
  if (hasToolCalls && hasText) {
    return (
      <>
        <ContentBubble message={message} />
        <ToolCallBubble tool_calls={message.tool_calls!} />
      </>
    );
  }

  if (hasToolCalls) {
    return <ToolCallBubble tool_calls={message.tool_calls!} />;
  }

  if (message.role === 'tool') {
    return <ToolOutputBubble message={message} />;
  }

  return <ContentBubble message={message} />;
};

export default MessageBubbleRouter;
