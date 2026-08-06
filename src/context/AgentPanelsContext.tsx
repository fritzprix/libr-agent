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

export type AgentPanelSide = 'left' | 'right';

const PANEL_SIDE: Record<AgentPanelId, AgentPanelSide> = {
  workspace: 'left',
  processes: 'left',
  planning: 'right',
};

interface PanelsState {
  open: Record<AgentPanelId, boolean>;
  lastClosedAt: Record<AgentPanelId, number>;
  /** Unread / attention indicators for panel header toggles (no auto-open). */
  attention: Record<AgentPanelId, boolean>;
}

const INITIAL_STATE: PanelsState = {
  open: {
    workspace: false,
    planning: false,
    processes: false,
  },
  lastClosedAt: {
    workspace: 0,
    planning: 0,
    processes: 0,
  },
  attention: {
    workspace: false,
    planning: false,
    processes: false,
  },
};

export function getAgentPanelSide(id: AgentPanelId): AgentPanelSide {
  return PANEL_SIDE[id];
}

/** Panels that share the same rail/sheet side are mutually exclusive when opening. */
export function getSiblingPanels(id: AgentPanelId): AgentPanelId[] {
  const side = PANEL_SIDE[id];
  return AGENT_PANEL_IDS.filter(
    (candidate) => candidate !== id && PANEL_SIDE[candidate] === side,
  );
}

interface AgentPanelsContextValue {
  isPanelOpen: (id: AgentPanelId) => boolean;
  openPanel: (id: AgentPanelId) => void;
  closePanel: (id: AgentPanelId) => void;
  togglePanel: (id: AgentPanelId) => void;
  closeAllPanels: () => void;
  /** Epoch ms when the panel was last closed by the user (0 if never). */
  getLastClosedAt: (id: AgentPanelId) => number;
  /** True when the panel has unviewed updates while closed. */
  hasPanelAttention: (id: AgentPanelId) => boolean;
  /** Mark a closed panel as having updates (notification dot). No-op if open. */
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

  const openPanel = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (current.open[id]) {
        return current;
      }

      const now = Date.now();
      const open: Record<AgentPanelId, boolean> = {
        ...current.open,
        [id]: true,
      };
      const lastClosedAt = { ...current.lastClosedAt };
      const attention = { ...current.attention, [id]: false };
      for (const sibling of getSiblingPanels(id)) {
        if (current.open[sibling]) {
          lastClosedAt[sibling] = now;
        }
        open[sibling] = false;
      }

      logger.info('Panel opened', {
        panel: id,
        closedSiblings: getSiblingPanels(id),
      });
      return { open, lastClosedAt, attention };
    });
  }, []);

  const closePanel = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (!current.open[id]) {
        return current;
      }
      logger.info('Panel closed', { panel: id });
      return {
        ...current,
        open: { ...current.open, [id]: false },
        lastClosedAt: { ...current.lastClosedAt, [id]: Date.now() },
      };
    });
  }, []);

  const togglePanel = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (current.open[id]) {
        logger.info('Panel toggled closed', { panel: id });
        return {
          ...current,
          open: { ...current.open, [id]: false },
          lastClosedAt: { ...current.lastClosedAt, [id]: Date.now() },
        };
      }

      const now = Date.now();
      const open: Record<AgentPanelId, boolean> = {
        ...current.open,
        [id]: true,
      };
      const lastClosedAt = { ...current.lastClosedAt };
      const attention = { ...current.attention, [id]: false };
      for (const sibling of getSiblingPanels(id)) {
        if (current.open[sibling]) {
          lastClosedAt[sibling] = now;
        }
        open[sibling] = false;
      }
      logger.info('Panel toggled open', {
        panel: id,
        closedSiblings: getSiblingPanels(id),
      });
      return { open, lastClosedAt, attention };
    });
  }, []);

  const closeAllPanels = useCallback(() => {
    setState((current) => {
      const anyOpen = AGENT_PANEL_IDS.some((id) => current.open[id]);
      if (!anyOpen) {
        return current;
      }
      logger.info('All panels closed');
      const now = Date.now();
      return {
        open: { ...INITIAL_STATE.open },
        lastClosedAt: {
          workspace: now,
          planning: now,
          processes: now,
        },
        attention: { ...INITIAL_STATE.attention },
      };
    });
  }, []);

  const isPanelOpen = useCallback(
    (id: AgentPanelId) => state.open[id],
    [state.open],
  );

  const getLastClosedAt = useCallback(
    (id: AgentPanelId) => state.lastClosedAt[id],
    [state.lastClosedAt],
  );

  const hasPanelAttention = useCallback(
    (id: AgentPanelId) => state.attention[id],
    [state.attention],
  );

  const markPanelAttention = useCallback((id: AgentPanelId) => {
    setState((current) => {
      if (current.open[id] || current.attention[id]) {
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
