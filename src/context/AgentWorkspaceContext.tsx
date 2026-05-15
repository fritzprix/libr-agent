import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentWorkspaceContext');

interface AgentWorkspaceContextValue {
  showWorkspacePanel: boolean;
  openWorkspacePanel: () => void;
  closeWorkspacePanel: () => void;
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

  const openWorkspacePanel = useCallback(() => {
    setShowWorkspacePanel((current) => {
      if (!current) {
        logger.info('Workspace panel opened');
      }
      return true;
    });
  }, []);

  const closeWorkspacePanel = useCallback(() => {
    setShowWorkspacePanel((current) => {
      if (current) {
        logger.info('Workspace panel closed');
      }
      return false;
    });
  }, []);

  const toggleWorkspacePanel = useCallback(() => {
    setShowWorkspacePanel((current) => {
      const next = !current;
      logger.info('Workspace panel toggled', {
        from: current,
        to: next,
      });
      return next;
    });
  }, []);

  const value = useMemo<AgentWorkspaceContextValue>(
    () => ({
      showWorkspacePanel,
      openWorkspacePanel,
      closeWorkspacePanel,
      toggleWorkspacePanel,
    }),
    [
      showWorkspacePanel,
      openWorkspacePanel,
      closeWorkspacePanel,
      toggleWorkspacePanel,
    ],
  );

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
