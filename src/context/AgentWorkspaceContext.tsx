import React, { createContext, useCallback, useContext, useState } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentWorkspaceContext');

interface AgentWorkspaceContextValue {
  showWorkspacePanel: boolean;
  toggleWorkspacePanel: () => void;
}

const AgentWorkspaceContext = createContext<
  AgentWorkspaceContextValue | undefined
>(undefined);

interface AgentWorkspaceProviderProps {
  children: React.ReactNode;
}

export function AgentWorkspaceProvider({
  children,
}: AgentWorkspaceProviderProps) {
  const [showWorkspacePanel, setShowWorkspacePanel] = useState(false);

  logger.debug('Provider render', { showWorkspacePanel });

  const toggleWorkspacePanel = useCallback(() => {
    const newValue = !showWorkspacePanel;
    logger.info('Workspace panel toggled', {
      from: showWorkspacePanel,
      to: newValue,
    });
    setShowWorkspacePanel(newValue);
  }, [showWorkspacePanel]);

  const value: AgentWorkspaceContextValue = {
    showWorkspacePanel,
    toggleWorkspacePanel,
  };

  return (
    <AgentWorkspaceContext.Provider value={value}>
      {children}
    </AgentWorkspaceContext.Provider>
  );
}

export function useAgentWorkspace(): AgentWorkspaceContextValue {
  const context = useContext(AgentWorkspaceContext);
  if (context === undefined) {
    throw new Error(
      'useAgentWorkspace must be used within an AgentWorkspaceProvider',
    );
  }
  return context;
}
