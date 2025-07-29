
import React from 'react';
import ContentBubble from './ContentBubble';
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';
import AttachmentBubble from './AttachmentBubble';

interface MessageBubbleRouterProps {
  message: {
    id: string;
    content: string;
    role: 'user' | 'assistant' | 'system' | 'tool';
    attachments?: {
      name: string;
      content: string;
    }[];
    tool_calls?: {
      id: string;
      type: 'function';
      function: { name: string; arguments: string };
    }[];
  };
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({ message }) => {
  if (message.tool_calls && message.tool_calls.length > 0) {
    return <ToolCallBubble tool_calls={message.tool_calls} />;
  }

  if (message.role === 'tool') {
    return <ToolOutputBubble content={message.content} />;
  }

  if (message.attachments && message.attachments.length > 0) {
    return <AttachmentBubble attachments={message.attachments} />;
  }

  return <ContentBubble content={message.content} />;
};

export default MessageBubbleRouter;
