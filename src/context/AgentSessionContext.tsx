import React, { createContext, useContext, useMemo, useCallback } from 'react';
import { getLogger } from '@/lib/logger';
import { safeInvoke } from '@/lib/backend/core';
import type { Page } from '@/lib/db/types';
import type { RustMessage, Message } from '@/models/chat';
import { rustMessageToMessage } from '@/models/chat';
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

const logger = getLogger('AgentSessionContext');

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

  const loadMessages = useCallback(
    async (sid: string) => {
      try {
        const page = await safeInvoke<Page<RustMessage>>('messages_get_page', {
          sessionId: sid,
          page: 1,
          pageSize: 1000,
        });

        const msgs: Message[] = page.items.map(rustMessageToMessage);

        stateProps.setters.setMessages(msgs);
      } catch (err) {
        logger.error('Failed to load messages', err);
      }
    },
    [stateProps.setters],
  );

  useAgentSessionEvents(sessionId, stateProps, {
    loadMessages,
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
      error: stateProps.state.error,
      llmError: stateProps.state.llmError,
      workflowStatus: stateProps.state.workflowStatus,
      workflowPhase: stateProps.state.workflowPhase,
      initializationStep: stateProps.state.initializationStep,
      pendingApprovals: stateProps.state.pendingApprovals,
      yoloModeEnabled: stateProps.state.yoloModeEnabled,
    }),
    [
      stateProps.state.session,
      stateProps.state.messages,
      stateProps.state.isSessionLoading,
      stateProps.state.error,
      stateProps.state.llmError,
      stateProps.state.workflowStatus,
      stateProps.state.workflowPhase,
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
