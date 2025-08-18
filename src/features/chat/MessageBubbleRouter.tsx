import { Message } from '@/models/chat';
import React from 'react';
import ContentBubble from './ContentBubble';
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';

interface MessageBubbleRouterProps {
  message: Message;
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
}) => {
  if (
    message.tool_calls &&
    Array.isArray(message.tool_calls) &&
    message.tool_calls.length > 0 &&
    message.tool_calls.every((tc) => tc && tc.function && tc.function.name)
  ) {
    return <ToolCallBubble tool_calls={message.tool_calls} />;
  }

  if (message.role === 'tool') {
    return <ToolOutputBubble message={message} />;
  }

  return <ContentBubble message={message} />;
};

export default MessageBubbleRouter;
