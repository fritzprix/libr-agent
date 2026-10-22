import React, { createContext, useContext, useMemo } from 'react';

import { resetAgentSessionListStartupCacheForTests } from './agent-session-list/startup-cache';
import type {
  AgentSessionListActionsContextValue,
  AgentSessionListStateContextValue,
} from './agent-session-list/types';
import { useSessionListEventSync } from './agent-session-list/useSessionListEventSync';
import { useSessionListLoaders } from './agent-session-list/useSessionListLoaders';
import { useSessionListMutations } from './agent-session-list/useSessionListMutations';
import { useSessionListStore } from './agent-session-list/useSessionListStore';

export function __resetAgentSessionListStartupCacheForTests() {
  resetAgentSessionListStartupCacheForTests();
}

// --- STATE CONTEXT ---
const AgentSessionListStateContext = createContext<
  AgentSessionListStateContextValue | undefined
>(undefined);

// --- ACTIONS CONTEXT ---
const AgentSessionListActionsContext = createContext<
  AgentSessionListActionsContextValue | undefined
>(undefined);

interface AgentSessionListProviderProps {
  children: React.ReactNode;
}

/**
 * AgentSessionListProvider
 *
 * Manages the global list of agent sessions.
 */
export function AgentSessionListProvider({
  children,
}: AgentSessionListProviderProps) {
  const store = useSessionListStore();
  const { sessions, notificationSessions, applySessionUpdate } = store;
  const {
    isSessionsListLoading,
    isLoadingMoreSessions,
    hasMoreSessions,
    loadingChildrenParentIds,
    loadSessions,
    loadMoreSessions,
    ensureChildrenLoaded,
  } = useSessionListLoaders(store);
  const {
    pendingApprovalKeysRef,
    createSession,
    deleteSession,
    deleteSessionOnly,
    toggleBookmark,
    renameSession,
    markSessionViewed,
    clearPendingApproval,
  } = useSessionListMutations(store);

  useSessionListEventSync({
    loadSessions,
    applySessionUpdate,
    clearPendingApproval,
    markSessionViewed,
    pendingApprovalKeysRef,
  });

  const stateValue = useMemo(
    () => ({
      sessions,
      notificationSessions,
      unreadNotificationCount: notificationSessions.length,
      isSessionsListLoading,
      hasMoreSessions,
      isLoadingMoreSessions,
      loadingChildrenParentIds,
    }),
    [
      hasMoreSessions,
      isLoadingMoreSessions,
      isSessionsListLoading,
      loadingChildrenParentIds,
      notificationSessions,
      sessions,
    ],
  );

  const actionsValue = useMemo(
    () => ({
      createSession,
      loadSessions,
      loadMoreSessions,
      ensureChildrenLoaded,
      deleteSession,
      deleteSessionOnly,
      toggleBookmark,
      renameSession,
      markSessionViewed,
      clearPendingApproval,
    }),
    [
      createSession,
      loadSessions,
      loadMoreSessions,
      ensureChildrenLoaded,
      deleteSession,
      deleteSessionOnly,
      toggleBookmark,
      renameSession,
      markSessionViewed,
      clearPendingApproval,
    ],
  );

  return (
    <AgentSessionListStateContext.Provider value={stateValue}>
      <AgentSessionListActionsContext.Provider value={actionsValue}>
        {children}
      </AgentSessionListActionsContext.Provider>
    </AgentSessionListStateContext.Provider>
  );
}

export function useAgentSessionListState(): AgentSessionListStateContextValue {
  const context = useContext(AgentSessionListStateContext);
  if (!context) {
    throw new Error(
      'useAgentSessionListState must be used within AgentSessionListProvider',
    );
  }
  return context;
}

export function useAgentSessionListActions(): AgentSessionListActionsContextValue {
  const context = useContext(AgentSessionListActionsContext);
  if (!context) {
    throw new Error(
      'useAgentSessionListActions must be used within AgentSessionListProvider',
    );
  }
  return context;
}
