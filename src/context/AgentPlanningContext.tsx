import React, { createContext, useCallback, useContext, useState } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentPlanningContext');

interface AgentPlanningContextValue {
  showPlanningPanel: boolean;
  togglePlanningPanel: () => void;
}

const AgentPlanningContext = createContext<
  AgentPlanningContextValue | undefined
>(undefined);

interface AgentPlanningProviderProps {
  children: React.ReactNode;
}

export function AgentPlanningProvider({
  children,
}: AgentPlanningProviderProps) {
  const [showPlanningPanel, setShowPlanningPanel] = useState(false);

  const togglePlanningPanel = useCallback(() => {
    const newValue = !showPlanningPanel;
    logger.info('Planning panel toggled', {
      from: showPlanningPanel,
      to: newValue,
    });
    setShowPlanningPanel(newValue);
  }, [showPlanningPanel]);

  const value: AgentPlanningContextValue = {
    showPlanningPanel,
    togglePlanningPanel,
  };

  return (
    <AgentPlanningContext.Provider value={value}>
      {children}
    </AgentPlanningContext.Provider>
  );
}

export function useAgentPlanning(): AgentPlanningContextValue {
  const context = useContext(AgentPlanningContext);
  if (context === undefined) {
    throw new Error(
      'useAgentPlanning must be used within an AgentPlanningProvider',
    );
  }
  return context;
}
