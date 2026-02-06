import { createContext, useContext, useEffect, useRef } from 'react';
import { useSettings } from '../SettingsContext';
import { useLLMState } from './useLLMState';
import { useCompletionExecutor } from './useCompletionExecutor';
import { useLLMListener } from './useLLMListener';
import type { LLMServiceContextValue, LLMServiceProviderProps } from './types';

export type { LLMServiceContextValue, LLMServiceProviderProps } from './types';
export * from './types'; // Re-export types for convenience

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

/**
 * Global LLM Service Provider
 * Lives at the App level and never unmounts
 * Provides centralized LLM execution for both UI and Agent workflows
 */
export function LLMServiceProvider({ children }: LLMServiceProviderProps) {
  const { value: settings } = useSettings();

  // Use ref to always access latest settings in event listeners
  const settingsRef = useRef(settings);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  // 1. Initialize State
  const state = useLLMState();

  // 2. Initialize Executor
  const executor = useCompletionExecutor(state, settingsRef);

  // 3. Initialize Listener
  useLLMListener(state, executor, settingsRef);

  const value: LLMServiceContextValue = {
    streamingMessages: state.streamingMessages,
    getSessionStatus: state.getSessionStatus,
    clearStreamingMessage: state.clearStreamingMessage,
    executeCompletionRequest: executor.executeCompletionRequest,
    setAgentMode: state.setAgentMode,
    getAgentMode: state.getAgentMode,
  };

  return (
    <LLMServiceContext.Provider value={value}>
      {children}
    </LLMServiceContext.Provider>
  );
}
