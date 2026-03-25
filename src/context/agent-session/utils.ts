import type { MessageError } from '@/models/chat';
import type { AgentRuntimeError } from '@/models/agent-ipc';

export function buildMessageError(
  error: string | AgentRuntimeError,
  fallbackType: MessageError['type'] = 'AI_SERVICE_ERROR',
): MessageError {
  if (typeof error !== 'string') {
    return error;
  }

  return {
    type: fallbackType,
    displayMessage: error,
    recoverable: true,
    details: {
      originalError: error,
      timestamp: new Date().toISOString(),
    },
  };
}
