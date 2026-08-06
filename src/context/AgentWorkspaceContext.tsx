/**
 * Compatibility facade over AgentPanelsContext.
 * Panel open state lives only in AgentPanelsProvider — do not add local state here.
 */
import { useMemo } from 'react';
import { useAgentPanels } from '@/context/AgentPanelsContext';

interface AgentWorkspaceContextValue {
  showWorkspacePanel: boolean;
  openWorkspacePanel: () => void;
  closeWorkspacePanel: () => void;
  toggleWorkspacePanel: () => void;
}

/** @deprecated Prefer AgentPanelsProvider — this no longer owns state. */
export function AgentWorkspaceProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}

/** @deprecated Prefer useAgentPanels() — this is a thin compatibility facade. */
export function useAgentWorkspace(): AgentWorkspaceContextValue {
  const { isPanelOpen, openPanel, closePanel, togglePanel } = useAgentPanels();

  return useMemo(
    () => ({
      showWorkspacePanel: isPanelOpen('workspace'),
      openWorkspacePanel: () => {
        openPanel('workspace');
      },
      closeWorkspacePanel: () => {
        closePanel('workspace');
      },
      toggleWorkspacePanel: () => {
        togglePanel('workspace');
      },
    }),
    [closePanel, isPanelOpen, openPanel, togglePanel],
  );
}
