import React, { createContext, useCallback, useContext } from 'react';
import { useChatState } from '../hooks/useChatState';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ChatWorkspaceContext');

interface ChatWorkspaceContextValue {
  showWorkspacePanel: boolean;
  toggleWorkspacePanel: () => void;
}

const ChatWorkspaceContext = createContext<
  ChatWorkspaceContextValue | undefined
>(undefined);

interface ChatWorkspaceProviderProps {
  children: React.ReactNode;
}

export function ChatWorkspaceProvider({
  children,
}: ChatWorkspaceProviderProps) {
  const { showWorkspacePanel, setShowWorkspacePanel } = useChatState();

  logger.info('CHAT_WORKSPACE_CONTEXT: Provider render', {
    showWorkspacePanel,
  });

  const toggleWorkspacePanel = useCallback(() => {
    const newValue = !showWorkspacePanel;
    logger.info('CHAT_WORKSPACE_CONTEXT: Workspace panel toggled', {
      from: showWorkspacePanel,
      to: newValue,
    });
    setShowWorkspacePanel(newValue);
  }, [showWorkspacePanel, setShowWorkspacePanel]);

  const value: ChatWorkspaceContextValue = {
    showWorkspacePanel,
    toggleWorkspacePanel,
  };

  return (
    <ChatWorkspaceContext.Provider value={value}>
      {children}
    </ChatWorkspaceContext.Provider>
  );
}

export function useChatWorkspace(): ChatWorkspaceContextValue {
  const context = useContext(ChatWorkspaceContext);
  if (context === undefined) {
    throw new Error(
      'useChatWorkspace must be used within a ChatWorkspaceProvider',
    );
  }
  return context;
}
