import React, { useState } from 'react';
import Chat from './Chat';
import StartSingleChatView from './StartSingleChatView';
import { useSessionContext } from '@/context/SessionContext';

interface ChatContainerProps {
  children?: React.ReactNode;
}

export default function ChatContainer({ children }: ChatContainerProps) {
  const [showAssistantManager, setShowAssistantManager] = useState(false);
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    return (
      <StartSingleChatView
        showAssistantManager={showAssistantManager}
        setShowAssistantManager={setShowAssistantManager}
      />
    );
  }

  return (
    <Chat
      showAssistantManager={showAssistantManager}
      setShowAssistantManager={setShowAssistantManager}
    >
      {children}
    </Chat>
  );
}
