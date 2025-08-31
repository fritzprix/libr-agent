import React from 'react';
import { ChatProvider } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatState } from './hooks/useChatState';
import { ChatHeader } from './components/ChatHeader';
import { ChatMessages } from './components/ChatMessages';
import { ChatStatusBar } from './components/ChatStatusBar';
import { ChatAttachedFiles } from './components/ChatAttachedFiles';
import { ChatInput } from './components/ChatInput';
import { ChatBottom } from './components/ChatBottom';
import ToolsModal from '../tools/ToolsModal';
import { TimeLocationSystemPrompt } from '../prompts/TimeLocationSystemPrompt';

interface ChatProps {
  children?: React.ReactNode;
}

function Chat({ children }: ChatProps) {
  const { showToolsDetail, setShowToolsDetail } = useChatState();
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }

  return (
    <ChatProvider>
      <TimeLocationSystemPrompt />
      <div className="h-full w-full font-mono flex flex-col rounded-lg overflow-hidden shadow-2xl">
        {children}
        <ToolsModal
          isOpen={showToolsDetail}
          onClose={() => setShowToolsDetail(false)}
        />
      </div>
    </ChatProvider>
  );
}

Chat.Header = ChatHeader;
Chat.Messages = ChatMessages;
Chat.StatusBar = ChatStatusBar;
Chat.AttachedFiles = ChatAttachedFiles;
Chat.Input = ChatInput;
Chat.Bottom = ChatBottom;

export default Chat;
