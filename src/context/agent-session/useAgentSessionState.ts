import { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import type { AgentSession } from '@/models/agent';
import type { Message, MessageError } from '@/models/chat';
import type { AgentRuntimeError } from '@/models/agent-ipc';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type { WorkflowPhase, PendingApproval } from './types';
import { buildMessageError } from './utils';

export function useAgentSessionState() {
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
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

  const setters = useMemo(
    () => ({
      setSession,
      setMessages,
      setIsSessionLoading,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setInitializationStep,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
    }),
    [
      setSession,
      setMessages,
      setIsSessionLoading,
      setError,
      setLlmError,
      setWorkflowStatus,
      setWorkflowPhase,
      setInitializationStep,
      setPendingApprovals,
      setYoloModeEnabled,
      applyLocalViewedAt,
      addMessage,
    ],
  );

  return {
    state: {
      session,
      messages,
      isSessionLoading,
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
