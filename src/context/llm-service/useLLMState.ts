import { useState, useRef, useCallback } from 'react';
import { Message } from '@/models/chat';
import { SessionStatus } from './types';
import { IAIService } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useLLMState');

export function useLLMState() {
  const [streamingMessages, setStreamingMessages] = useState<
    Map<string, Partial<Message>>
  >(new Map());
  const [sessionStatuses, setSessionStatuses] = useState<
    Map<string, SessionStatus>
  >(new Map());
  const [sessionAgentModes, setSessionAgentModes] = useState<
    Map<string, boolean>
  >(new Map());

  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup - using number for browser compatibility
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track listener setup to prevent duplicate registration in React Strict Mode
  const listenerSetupRef = useRef(false);

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

  const setAgentMode = useCallback((sessionId: string, enabled: boolean) => {
    setSessionAgentModes((prev) => {
      const next = new Map(prev);
      next.set(sessionId, enabled);
      return next;
    });
  }, []);

  /**
   * Clear streaming message for a specific session
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
    getSessionStatus,
    updateSessionStatus,
    sessionAgentModes,
    getAgentMode,
    setAgentMode,
    clearStreamingMessage,
    activeServicesRef,
    abortControllersRef,
    timeoutsRef,
    listenerSetupRef,
  };
}
