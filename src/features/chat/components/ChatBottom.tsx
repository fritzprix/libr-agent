import React, { useState } from 'react';
import { ChatStatusBar } from './ChatStatusBar';
import { ChatAttachedFiles } from './ChatAttachedFiles';
import { ChatInput } from './ChatInput';
import ToolsModal from '../../tools/ToolsModal';

interface ChatBottomProps {
  children?: React.ReactNode;
}

export function ChatBottom({ children }: ChatBottomProps) {
  const [showToolsDetail, setShowToolsDetail] = useState(false);

  return (
    <div className="flex-shrink-0">
      <ChatStatusBar onShowTools={() => setShowToolsDetail(true)} />
      <ChatAttachedFiles />
      <ChatInput />
      <ToolsModal
        isOpen={showToolsDetail}
        onClose={() => setShowToolsDetail(false)}
      />
      {children}
    </div>
  );
}
