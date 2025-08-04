import React from 'react';
import ContentBubble from './ContentBubble';
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';
import AttachmentBubble from './AttachmentBubble';
import { Message } from '@/models/chat';

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
    message.tool_calls.every(tc => tc && tc.function && tc.function.name)
  ) {
    return <ToolCallBubble tool_calls={message.tool_calls} />;
  }

  if (message.role === 'tool') {
    return <ToolOutputBubble message={message} />;
  }

  if (message.attachments && message.attachments.length > 0) {
    return <AttachmentBubble attachments={message.attachments} />;
  }

  return <ContentBubble message={message} />;
};

export default MessageBubbleRouter;
