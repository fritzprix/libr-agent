import { useState, useRef, useCallback } from 'react';
import type { Message } from '@/models/chat';
import type { IAIService } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import type { SessionStatus } from './types';

const logger = getLogger('LLMServiceState');

export function useLLMState() {
  const [streamingMessages, setStreamingMessages] = useState<
    Map<string, Partial<Message>>
  >(new Map());
  const [sessionStatuses, setSessionStatuses] = useState<
    Map<string, SessionStatus>
  >(new Map());

  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup - using number for browser compatibility
  const timeoutsRef = useRef<Map<string, number>>(new Map());

  const [sessionAgentModes, setSessionAgentModes] = useState<
    Map<string, boolean>
  >(new Map());

  const setAgentMode = useCallback((sessionId: string, enabled: boolean) => {
    setSessionAgentModes((prev) => {
      const next = new Map(prev);
      next.set(sessionId, enabled);
      return next;
    });
  }, []);

  /**
   * Get session status
   */
  const getSessionStatus = useCallback(
    (sessionId: string): SessionStatus => {
      return sessionStatuses.get(sessionId) ?? 'idle';
    },
    [sessionStatuses],
  );

  /**
   * Update session status
   */
  const updateSessionStatus = useCallback(
    (sessionId: string, status: SessionStatus) => {
      setSessionStatuses((prev) => {
        const next = new Map(prev);
        next.set(sessionId, status);
        return next;
      });
    },
    [],
  );

  const getAgentMode = useCallback(
    (sessionId: string) => {
      return sessionAgentModes.get(sessionId) ?? false;
    },
    [sessionAgentModes],
  );

  /**
   * Clear streaming message for a specific session
   * This is called by AgentChatContext after persisting the message
   */
  const clearStreamingMessage = useCallback((sessionId: string) => {
    logger.debug('Clearing streaming message', { sessionId });

    setStreamingMessages((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

  return {
    streamingMessages,
    setStreamingMessages,
    sessionStatuses,
    setSessionStatuses,
    activeServicesRef,
    abortControllersRef,
    timeoutsRef,
    sessionAgentModes,
    setAgentMode,
    getAgentMode,
    getSessionStatus,
    updateSessionStatus,
    clearStreamingMessage,
  };
}
