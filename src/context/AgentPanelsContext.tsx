import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentPanelsContext');

export const AGENT_PANEL_IDS = ['workspace', 'planning', 'processes'] as const;

export type AgentPanelId = (typeof AGENT_PANEL_IDS)[number];

interface PanelsState {
  shellOpen: boolean;
  activeTab: AgentPanelId;
  /** Epoch ms when the shell was last closed (0 if never). */
  lastClosedAt: number;
  /** Unread indicators per tab (no auto-open). */
  attention: Record<AgentPanelId, boolean>;
}

const INITIAL_STATE: PanelsState = {
  shellOpen: false,
  activeTab: 'workspace',
  lastClosedAt: 0,
  attention: {
    workspace: false,
    planning: false,
    processes: false,
  },
};

interface AgentPanelsContextValue {
  isShellOpen: () => boolean;
  openShell: (tab?: AgentPanelId) => void;
  closeShell: () => void;
  toggleShell: () => void;
  activeTab: AgentPanelId;
  setActiveTab: (id: AgentPanelId) => void;
  /** True when the shell is open and `id` is the active tab. */
  isPanelOpen: (id: AgentPanelId) => boolean;
  /** Opens the shell on `id` (clears that tab's attention). */
  openPanel: (id: AgentPanelId) => void;
  /** Closes the shell (argument ignored; kept for facade call sites). */
  closePanel: (id?: AgentPanelId) => void;
  /**
   * If shell is open on `id`, close it; otherwise open shell on `id`.
   */
  togglePanel: (id: AgentPanelId) => void;
  closeAllPanels: () => void;
  /** Epoch ms when the shell was last closed (0 if never). */
  getLastClosedAt: () => number;
  hasPanelAttention: (id: AgentPanelId) => boolean;
  /**
   * Mark a tab as having updates when the shell is closed or another tab is
   * active. No-op when that tab is currently visible.
   */
  markPanelAttention: (id: AgentPanelId) => void;
  clearPanelAttention: (id: AgentPanelId) => void;
}

const AgentPanelsContext = createContext<AgentPanelsContextValue | undefined>(
  undefined,
);

interface AgentPanelsProviderProps {
  children: React.ReactNode;
}

export function AgentPanelsProvider({ children }: AgentPanelsProviderProps) {
  const [state, setState] = useState<PanelsState>(INITIAL_STATE);

  const openShell = useCallback((tab?: AgentPanelId) => {
    setState((current) => {
      const nextTab = tab ?? current.activeTab;
      logger.info('Shell opened', { tab: nextTab });
      return {
        ...current,
        shellOpen: true,
        activeTab: nextTab,
        attention: { ...current.attention, [nextTab]: false },
      };
    });
  }, []);

  const closeShell = useCallback(() => {
    setState((current) => {
      if (!current.shellOpen) {
        return current;
      }
      logger.info('Shell closed');
      return {
        ...current,
        shellOpen: false,
        lastClosedAt: Date.now(),
      };
    });
  }, []);

  const toggleShell = useCallback(() => {
    setState((current) => {
      if (current.shellOpen) {
        logger.info('Shell toggled closed');
        return {
          ...current,
          shellOpen: false,
          lastClosedAt: Date.now(),
        };
      }
      logger.info('Shell toggled open', { tab: current.activeTab });
      return {
        ...current,
        shellOpen: true,
        attention: { ...current.attention, [current.activeTab]: false },
      };
    });
  }, []);

  const setActiveTab = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (current.activeTab === id && current.attention[id] === false) {
        return current.shellOpen ? current : { ...current, shellOpen: true };
      }
      logger.info('Active tab set', { tab: id });
      return {
        ...current,
        shellOpen: true,
        activeTab: id,
        attention: { ...current.attention, [id]: false },
      };
    });
  }, []);

  const openPanel = useCallback((id: AgentPanelId) => {
    setState((current) => {
      logger.info('Panel opened via tab', { tab: id });
      return {
        ...current,
        shellOpen: true,
        activeTab: id,
        attention: { ...current.attention, [id]: false },
      };
    });
  }, []);

  const closePanel = useCallback((id?: AgentPanelId) => {
    void id;
    setState((current) => {
      if (!current.shellOpen) {
        return current;
      }
      logger.info('Panel/shell closed');
      return {
        ...current,
        shellOpen: false,
        lastClosedAt: Date.now(),
      };
    });
  }, []);

  const togglePanel = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (current.shellOpen && current.activeTab === id) {
        logger.info('Panel toggled closed', { tab: id });
        return {
          ...current,
          shellOpen: false,
          lastClosedAt: Date.now(),
        };
      }
      logger.info('Panel toggled open', { tab: id });
      return {
        ...current,
        shellOpen: true,
        activeTab: id,
        attention: { ...current.attention, [id]: false },
      };
    });
  }, []);

  const closeAllPanels = useCallback(() => {
    setState((current) => {
      if (!current.shellOpen) {
        return {
          ...current,
          attention: { ...INITIAL_STATE.attention },
        };
      }
      logger.info('All panels closed');
      return {
        ...current,
        shellOpen: false,
        lastClosedAt: Date.now(),
        attention: { ...INITIAL_STATE.attention },
      };
    });
  }, []);

  const isShellOpen = useCallback(() => state.shellOpen, [state.shellOpen]);

  const isPanelOpen = useCallback(
    (id: AgentPanelId) => state.shellOpen && state.activeTab === id,
    [state.activeTab, state.shellOpen],
  );

  const getLastClosedAt = useCallback(
    () => state.lastClosedAt,
    [state.lastClosedAt],
  );

  const hasPanelAttention = useCallback(
    (id: AgentPanelId) => state.attention[id],
    [state.attention],
  );

  const markPanelAttention = useCallback((id: AgentPanelId) => {
    setState((current) => {
      const viewingTab = current.shellOpen && current.activeTab === id;
      if (viewingTab || current.attention[id]) {
        return current;
      }
      logger.info('Panel attention marked', { panel: id });
      return {
        ...current,
        attention: { ...current.attention, [id]: true },
      };
    });
  }, []);

  const clearPanelAttention = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (!current.attention[id]) {
        return current;
      }
      return {
        ...current,
        attention: { ...current.attention, [id]: false },
      };
    });
  }, []);

  const value = useMemo<AgentPanelsContextValue>(
    () => ({
      isShellOpen,
      openShell,
      closeShell,
      toggleShell,
      activeTab: state.activeTab,
      setActiveTab,
      isPanelOpen,
      openPanel,
      closePanel,
      togglePanel,
      closeAllPanels,
      getLastClosedAt,
      hasPanelAttention,
      markPanelAttention,
      clearPanelAttention,
    }),
    [
      isShellOpen,
      openShell,
      closeShell,
      toggleShell,
      state.activeTab,
      setActiveTab,
      isPanelOpen,
      openPanel,
      closePanel,
      togglePanel,
      closeAllPanels,
      getLastClosedAt,
      hasPanelAttention,
      markPanelAttention,
      clearPanelAttention,
    ],
  );

  return (
    <AgentPanelsContext.Provider value={value}>
      {children}
    </AgentPanelsContext.Provider>
  );
}

export function useAgentPanels(): AgentPanelsContextValue {
  const context = useContext(AgentPanelsContext);
  if (context === undefined) {
    throw new Error(
      'useAgentPanels must be used within an AgentPanelsProvider',
    );
  }
  return context;
}
