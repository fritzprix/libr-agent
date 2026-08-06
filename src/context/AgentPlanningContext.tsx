/**
 * Compatibility facade over AgentPanelsContext.
 * Panel open state lives only in AgentPanelsProvider — do not add local state here.
 */
import { useMemo } from 'react';
import { useAgentPanels } from '@/context/AgentPanelsContext';

interface AgentPlanningContextValue {
  showPlanningPanel: boolean;
  openPlanningPanel: () => void;
  closePlanningPanel: () => void;
  togglePlanningPanel: () => void;
}

/** @deprecated Prefer AgentPanelsProvider — this no longer owns state. */
export function AgentPlanningProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}

/** @deprecated Prefer useAgentPanels() — this is a thin compatibility facade. */
export function useAgentPlanning(): AgentPlanningContextValue {
  const { isPanelOpen, openPanel, closePanel, togglePanel } = useAgentPanels();

  return useMemo(
    () => ({
      showPlanningPanel: isPanelOpen('planning'),
      openPlanningPanel: () => {
        openPanel('planning');
      },
      closePlanningPanel: () => {
        closePanel('planning');
      },
      togglePlanningPanel: () => {
        togglePanel('planning');
      },
    }),
    [closePanel, isPanelOpen, openPanel, togglePanel],
  );
}
