import { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import type { AgentSession } from '@/models/agent';
import type { Message, MessageError } from '@/models/chat';
import type {
  AgentRuntimeError,
  MessageCursor,
  SessionRuntimeState,
} from '@/models/agent-ipc';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type { WorkflowPhase, PendingApproval } from './types';
import { buildMessageError } from './utils';

const DEFAULT_RUNTIME_STATE: SessionRuntimeState = {
  phase: 'not_started',
  proxy: {
    exists: false,
    mode: 'none',
    ready: false,
  },
  initialization: {
    result: 'pending',
  },
  servers: [],
};

export function useAgentSessionState() {
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoadingOlderMessages, setIsLoadingOlderMessages] = useState(false);
  const [hasOlderMessages, setHasOlderMessages] = useState(false);
  const [oldestMessageCursor, setOldestMessageCursor] =
    useState<MessageCursor | null>(null);
  const [error, setErrorState] = useState<MessageError | null>(null);
  const [llmError, setLlmError] = useState<MessageError | null>(null);
  const [workflowStatus, setWorkflowStatus] = useState<
    'idle' | 'busy' | 'paused' | 'error'
  >('idle');
  const [workflowPhase, setWorkflowPhase] = useState<WorkflowPhase>('idle');
  const [runtimeState, setRuntimeState] = useState<SessionRuntimeState>(
    DEFAULT_RUNTIME_STATE,
  );
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>(
    [],
  );
  const [yoloModeEnabled, setYoloModeEnabled] = useState(false);

  const yoloModeRef = useRef(yoloModeEnabled);
  const workflowPhaseRef = useRef(workflowPhase);

  useEffect(() => {
    yoloModeRef.current = yoloModeEnabled;
  }, [yoloModeEnabled]);

  useEffect(() => {
    workflowPhaseRef.current = workflowPhase;
  }, [workflowPhase]);

  const setError = useCallback(
    (nextError: string | AgentRuntimeError | null) => {
      setErrorState(nextError ? buildMessageError(nextError) : null);
    },
    [],
  );

  const applyLocalViewedAt = useCallback((viewedAt: Date) => {
    setSession((prev) =>
      prev ? applyViewedAtToSession(prev, viewedAt) : prev,
    );
  }, []);

  const addMessage = useCallback((message: Message) => {
    setMessages((prev) => {
      if (prev.some((m) => m.id === message.id)) return prev;
      return [...prev, message];
    });
  }, []);

  const prependMessages = useCallback((olderMessages: Message[]) => {
    if (olderMessages.length === 0) {
      return;
    }

    setMessages((prev) => {
      const existingIds = new Set(prev.map((message) => message.id));
      const dedupedOlder = olderMessages.filter(
        (message) => !existingIds.has(message.id),
      );

      if (dedupedOlder.length === 0) {
        return prev;
      }

      return [...dedupedOlder, ...prev];
    });
  }, []);

  const setters = useMemo(
    () => ({
      setSession,
      setMessages,
      setIsLoadingOlderMessages,
      setHasOlderMessages,
      setOldestMessageCursor,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setRuntimeState,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
      prependMessages,
    }),
    [
      setSession,
      setMessages,
      setIsLoadingOlderMessages,
      setHasOlderMessages,
      setOldestMessageCursor,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setRuntimeState,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
      prependMessages,
    ],
  );

  const initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null = runtimeState.initialization.currentStep
    ? {
        step: runtimeState.initialization.currentStep,
        status:
          runtimeState.phase === 'failed'
            ? 'error'
            : runtimeState.phase === 'ready' ||
                runtimeState.phase === 'degraded'
              ? 'complete'
              : 'running',
      }
    : null;

  return {
    state: {
      session,
      messages,
      isSessionLoading:
        runtimeState.phase === 'hydrating' ||
        runtimeState.phase === 'initializing',
      isLoadingOlderMessages,
      hasOlderMessages,
      oldestMessageCursor,
      error,
      llmError,
      workflowStatus,
      workflowPhase,
      runtimeState,
      initializationStep,
      pendingApprovals,
      yoloModeEnabled,
    },
    refs: {
      yoloModeRef,
      workflowPhaseRef,
    },
    setters,
  };
}
