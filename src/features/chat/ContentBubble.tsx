import React from 'react';
import type { Message } from '@/models/chat';
import MessageRenderer from '@/components/MessageRenderer';

interface ContentBubbleProps {
  message: Message;
}

const ContentBubble: React.FC<ContentBubbleProps> = ({ message }) => {
  return (
    <MessageRenderer message={message} className="text-sm leading-relaxed" />
  );
};

export default ContentBubble;
