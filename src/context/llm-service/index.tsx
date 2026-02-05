import { createContext, useContext, useEffect, useRef, ReactNode } from 'react';
import { useSettings } from '@/context/SettingsContext';
import { LLMServiceContextValue } from './types';
import { useLLMState } from './useLLMState';
import { useCompletionExecutor } from './useCompletionExecutor';
import { useLLMListener } from './useLLMListener';

// Export types
export type {
  CompletionRequest,
  SessionStatus,
  LLMServiceContextValue,
} from './types';

const LLMServiceContext = createContext<LLMServiceContextValue | undefined>(
  undefined,
);

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

export function LLMServiceProvider({ children }: LLMServiceProviderProps) {
  const { value: settings } = useSettings();

  // Use ref to always access latest settings in event listeners
  const settingsRef = useRef(settings);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  // State Management
  const {
    streamingMessages,
    setStreamingMessages,
    // sessionStatuses, // Internal only
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
  } = useLLMState();

  // Execution Logic
  const executeCompletionRequest = useCompletionExecutor({
    settingsRef,
    streamingMessages,
    setStreamingMessages,
    updateSessionStatus,
    sessionAgentModes,
    activeServicesRef,
    abortControllersRef,
    timeoutsRef,
  });

  // Event Listener
  useLLMListener({
    listenerSetupRef,
    settingsRef,
    setStreamingMessages,
    executeCompletionRequest,
    abortControllersRef,
    timeoutsRef,
    activeServicesRef,
  });

  const value: LLMServiceContextValue = {
    streamingMessages,
    getSessionStatus,
    clearStreamingMessage,
    executeCompletionRequest,
    setAgentMode,
    getAgentMode,
  };

  return (
    <LLMServiceContext.Provider value={value}>
      {children}
    </LLMServiceContext.Provider>
  );
}
