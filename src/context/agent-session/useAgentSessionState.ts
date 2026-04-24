import { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import type { AgentSession } from '@/models/agent';
import type { Message, MessageError } from '@/models/chat';
import type { AgentRuntimeError } from '@/models/agent-ipc';
import type { MessageCursor } from '@/lib/backend/messages';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type { WorkflowPhase, PendingApproval } from './types';
import { buildMessageError } from './utils';

export function useAgentSessionState() {
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
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
  const [initializationStep, setInitializationStep] = useState<{
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null>(null);
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
      setIsSessionLoading,
      setIsLoadingOlderMessages,
      setHasOlderMessages,
      setOldestMessageCursor,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setInitializationStep,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
      prependMessages,
    }),
    [
      setSession,
      setMessages,
      setIsSessionLoading,
      setIsLoadingOlderMessages,
      setHasOlderMessages,
      setOldestMessageCursor,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setInitializationStep,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
      prependMessages,
    ],
  );

  return {
    state: {
      session,
      messages,
      isSessionLoading,
      isLoadingOlderMessages,
      hasOlderMessages,
      oldestMessageCursor,
      error,
      llmError,
      workflowStatus,
      workflowPhase,
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
