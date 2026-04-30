import React, { createContext, useContext, useMemo, useCallback } from 'react';
import { useAgentSessionListActions } from './AgentSessionListContext';
import { useAgentSessionState as useAgentSessionStateLogic } from './agent-session/useAgentSessionState';
import { useAgentSessionEvents } from './agent-session/useAgentSessionEvents';
import { useAgentSessionActionsLogic } from './agent-session/useAgentSessionActions';
import { useLLMService } from './LLMServiceContext';
import type {
  AgentSessionStateContextValue,
  AgentSessionActionsContextValue,
} from './agent-session/types';

// Re-export types so we don't break existing imports
export * from './agent-session/types';

const AgentSessionStateContext = createContext<
  AgentSessionStateContextValue | undefined
>(undefined);
const AgentSessionActionsContext = createContext<
  AgentSessionActionsContextValue | undefined
>(undefined);

interface AgentSessionProviderProps {
  children: React.ReactNode;
  sessionId: string;
}

export function AgentSessionProvider({
  children,
  sessionId,
}: AgentSessionProviderProps) {
  const { markSessionViewed, clearPendingApproval } =
    useAgentSessionListActions();
  const { refreshCompactedRange } = useLLMService();
  const stateProps = useAgentSessionStateLogic();

  React.useEffect(() => {
    void refreshCompactedRange(sessionId);
  }, [refreshCompactedRange, sessionId]);

  const persistViewedAt = useCallback(
    async (viewedAt = new Date()) => {
      stateProps.setters.applyLocalViewedAt(viewedAt);
      await markSessionViewed(sessionId, viewedAt);
    },
    [stateProps.setters, markSessionViewed, sessionId],
  );

  const acknowledgeSessionAttention = useCallback(
    async (viewedAt = new Date()) => {
      await persistViewedAt(viewedAt);
    },
    [persistViewedAt],
  );

  useAgentSessionEvents(sessionId, stateProps, {
    persistViewedAt,
  });

  const customActions = useAgentSessionActionsLogic(sessionId, stateProps, {
    acknowledgeSessionAttention,
    clearPendingApproval,
  });

  const stateValue: AgentSessionStateContextValue = useMemo(
    () => ({
      session: stateProps.state.session,
      messages: stateProps.state.messages,
      isSessionLoading: stateProps.state.isSessionLoading,
      isLoadingOlderMessages: stateProps.state.isLoadingOlderMessages,
      hasOlderMessages: stateProps.state.hasOlderMessages,
      error: stateProps.state.error,
      llmError: stateProps.state.llmError,
      workflowStatus: stateProps.state.workflowStatus,
      workflowPhase: stateProps.state.workflowPhase,
      runtimeState: stateProps.state.runtimeState,
      initializationStep: stateProps.state.initializationStep,
      pendingApprovals: stateProps.state.pendingApprovals,
      yoloModeEnabled: stateProps.state.yoloModeEnabled,
    }),
    [
      stateProps.state.session,
      stateProps.state.messages,
      stateProps.state.isSessionLoading,
      stateProps.state.isLoadingOlderMessages,
      stateProps.state.hasOlderMessages,
      stateProps.state.error,
      stateProps.state.llmError,
      stateProps.state.workflowStatus,
      stateProps.state.workflowPhase,
      stateProps.state.runtimeState,
      stateProps.state.initializationStep,
      stateProps.state.pendingApprovals,
      stateProps.state.yoloModeEnabled,
    ],
  );

  const actionsValue: AgentSessionActionsContextValue = useMemo(
    () => ({
      ...customActions,
      addMessage: stateProps.setters.addMessage,
      setError: stateProps.setters.setError,
    }),
    [customActions, stateProps.setters.addMessage, stateProps.setters.setError],
  );

  return (
    <AgentSessionStateContext.Provider value={stateValue}>
      <AgentSessionActionsContext.Provider value={actionsValue}>
        {children}
      </AgentSessionActionsContext.Provider>
    </AgentSessionStateContext.Provider>
  );
}

export function useAgentSessionState(): AgentSessionStateContextValue {
  const context = useContext(AgentSessionStateContext);
  if (!context) {
    throw new Error(
      'useAgentSessionState must be used within AgentSessionProvider',
    );
  }
  return context;
}

export function useOptionalAgentSessionState():
  | AgentSessionStateContextValue
  | undefined {
  return useContext(AgentSessionStateContext);
}

export function useAgentSessionActions(): AgentSessionActionsContextValue {
  const context = useContext(AgentSessionActionsContext);
  if (!context) {
    throw new Error(
      'useAgentSessionActions must be used within AgentSessionProvider',
    );
  }
  return context;
}

export function useAgentSession() {
  const state = useAgentSessionState();
  const actions = useAgentSessionActions();

  return {
    ...state,
    ...actions,
  };
}
