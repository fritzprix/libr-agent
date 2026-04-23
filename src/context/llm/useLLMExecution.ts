import React from 'react';

import type { SessionStatus } from './types';
import type { Message } from '@/models/chat';
import type { Settings } from '@/lib/services/settings-service';

import { useLLMExecutionState } from './useLLMExecutionState';
import { useExecuteCompletion } from './useExecuteCompletion';

interface UseLLMExecutionProps {
  settingsRef: React.MutableRefObject<Settings>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => void;
}

export function useLLMExecution(props: UseLLMExecutionProps) {
  const state = useLLMExecutionState();
  const exec = useExecuteCompletion(props);

  return {
    ...state,
    ...exec,
  };
}
