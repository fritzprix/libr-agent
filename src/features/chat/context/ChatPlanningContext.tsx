import React, { createContext, useContext, useCallback } from 'react';
import { useChatState } from '../hooks/useChatState';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ChatPlanningContext');

interface ChatPlanningContextValue {
  showPlanningPanel: boolean;
  togglePlanningPanel: () => void;
}

const ChatPlanningContext = createContext<ChatPlanningContextValue | undefined>(
  undefined,
);

interface ChatPlanningProviderProps {
  children: React.ReactNode;
}

export function ChatPlanningProvider({ children }: ChatPlanningProviderProps) {
  const { showPlanningPanel, setShowPlanningPanel } = useChatState();

  logger.info('CHAT_PLANNING_CONTEXT: Provider render', {
    showPlanningPanel,
  });

  const togglePlanningPanel = useCallback(() => {
    const newValue = !showPlanningPanel;
    logger.info('CHAT_PLANNING_CONTEXT: Planning panel toggled', {
      from: showPlanningPanel,
      to: newValue,
    });
    setShowPlanningPanel(newValue);
  }, [showPlanningPanel, setShowPlanningPanel]);

  const value: ChatPlanningContextValue = {
    showPlanningPanel,
    togglePlanningPanel,
  };

  return (
    <ChatPlanningContext.Provider value={value}>
      {children}
    </ChatPlanningContext.Provider>
  );
}

export function useChatPlanning(): ChatPlanningContextValue {
  const context = useContext(ChatPlanningContext);
  if (context === undefined) {
    throw new Error(
      'useChatPlanning must be used within a ChatPlanningProvider',
    );
  }
  return context;
}
