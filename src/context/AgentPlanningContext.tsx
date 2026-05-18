import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentPlanningContext');

interface AgentPlanningContextValue {
  showPlanningPanel: boolean;
  openPlanningPanel: () => void;
  closePlanningPanel: () => void;
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

  const openPlanningPanel = useCallback(() => {
    setShowPlanningPanel((current) => {
      if (!current) {
        logger.info('Planning panel opened');
      }
      return true;
    });
  }, []);

  const closePlanningPanel = useCallback(() => {
    setShowPlanningPanel((current) => {
      if (current) {
        logger.info('Planning panel closed');
      }
      return false;
    });
  }, []);

  const togglePlanningPanel = useCallback(() => {
    setShowPlanningPanel((current) => {
      const next = !current;
      logger.info('Planning panel toggled', {
        from: current,
        to: next,
      });
      return next;
    });
  }, []);

  const value = useMemo<AgentPlanningContextValue>(
    () => ({
      showPlanningPanel,
      openPlanningPanel,
      closePlanningPanel,
      togglePlanningPanel,
    }),
    [
      showPlanningPanel,
      openPlanningPanel,
      closePlanningPanel,
      togglePlanningPanel,
    ],
  );

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
