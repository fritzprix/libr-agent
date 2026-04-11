import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';
import type { Message } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useSettings } from './SettingsContext';
import { useLLMExecution } from './llm/useLLMExecution';
import { useLLMListener } from './llm/useLLMListener';
import type { LLMServiceContextValue, SessionStatus } from './llm/types';

// Re-export types for consumers
export type {
  CompletionRequest,
  SessionStatus,
  LLMServiceContextValue,
} from './llm/types';

const logger = getLogger('LLMServiceContext');

const LLMServiceContext = createContext<LLMServiceContextValue | undefined>(
  undefined,
);

/**
 * Hook to access LLM Service Context
 */
export function useLLMService(): LLMServiceContextValue {
  const context = useContext(LLMServiceContext);
  if (!context) {
    throw new Error('useLLMService must be used within LLMServiceProvider');
  }
  return context;
}

interface LLMServiceProviderProps {
  children: ReactNode;
}

/**
 * Global LLM Service Provider
 * Lives at the App level and never unmounts
 * Provides centralized LLM execution for both UI and Agent workflows
 */
export function LLMServiceProvider({ children }: LLMServiceProviderProps) {
  const { value: settings } = useSettings();

  // Use ref to always access latest settings in event listeners
  const settingsRef = useRef(settings);
  const prevStrategyRef = useRef(settings.contextStrategy);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  const [streamingMessages, setStreamingMessages] = useState<
    Map<string, Partial<Message>>
  >(new Map());
  const [sessionStatuses, setSessionStatuses] = useState<
    Map<string, SessionStatus>
  >(new Map());

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

  // Use extracted hooks
  const {
    executeCompletionRequest,
    cancelCompletionRequest,
    isCompacting,
    isAwaitingCompact,
    getCompactionPressure,
    getCompactedRange,
    clearSessionState,
    clearAllCompactState,
    clearCompactionPressureForSession,
    setCompactionPressureMap,
    setCompacting,
    setCompactedRange,
    setAwaitingCompact,
  } = useLLMExecution({
    settingsRef,
    streamingMessages,
    setStreamingMessages,
    updateSessionStatus,
  });

  // When the context strategy changes, purge all in-memory compact state so
  // stale caches, pending resolvers, and UI badges don't leak across modes.
  useEffect(() => {
    const prev = prevStrategyRef.current;
    const next = settings.contextStrategy;
    if (prev !== next) {
      prevStrategyRef.current = next;
      clearAllCompactState();
      logger.info('Context strategy changed — compact state cleared', {
        from: prev,
        to: next,
      });
    }
  }, [settings.contextStrategy, clearAllCompactState]);

  useLLMListener({
    settingsRef,
    executeCompletionRequest,
    setStreamingMessages,
    setCompactionPressureForSession: (sessionId, pressure) => {
      setCompactionPressureMap((prev) => {
        const next = new Map(prev);
        next.set(sessionId, pressure);
        return next;
      });
    },
    clearCompactionPressureForSession,
    setCompactingFromEvent: setCompacting,
    setCompactedRangeForSession: setCompactedRange,
    setAwaitingCompactForSession: setAwaitingCompact,
  });

  const value: LLMServiceContextValue = {
    streamingMessages,
    getSessionStatus,
    clearStreamingMessage,
    executeCompletionRequest,
    cancelCompletionRequest,
    isCompacting,
    isAwaitingCompact,
    getCompactionPressure,
    getCompactedRange,
    clearSessionState,
    clearAllCompactState,
  };

  return (
    <LLMServiceContext.Provider value={value}>
      {children}
    </LLMServiceContext.Provider>
  );
}
