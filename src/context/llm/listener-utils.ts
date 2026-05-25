import {
  isSpendingCapError,
  normalizeAIServiceError,
} from '@/lib/ai-service/utils';
import type { AgentRuntimeError } from '@/models/agent-ipc';
import type { MessageError } from '@/models/chat';

function isMessageError(error: unknown): error is MessageError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'displayMessage' in error &&
    typeof error.displayMessage === 'string' &&
    'type' in error &&
    typeof error.type === 'string' &&
    'recoverable' in error &&
    typeof error.recoverable === 'boolean'
  );
}

export function toAgentRuntimeError(error: unknown): AgentRuntimeError {
  if (isMessageError(error)) {
    return error;
  }

  const normalizedAiError = normalizeAIServiceError(error);
  if (normalizedAiError) {
    return {
      type: normalizedAiError.type,
      displayMessage: normalizedAiError.displayMessage,
      recoverable: normalizedAiError.recoverable,
      details: {
        originalError: error instanceof Error ? error.message : error,
        errorCode: normalizedAiError.errorCode,
        timestamp: new Date().toISOString(),
      },
    };
  }

  const displayMessage =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : String(error);

  return {
    type: 'AI_SERVICE_ERROR',
    displayMessage,
    recoverable: true,
    details: {
      originalError: error instanceof Error ? error.message : error,
      timestamp: new Date().toISOString(),
    },
  };
}

export function shouldBypassRetryAndFallback(error: unknown): boolean {
  if (toAgentRuntimeError(error).type === 'CONTEXT_LIMIT_ERROR') {
    return true;
  }

  return isSpendingCapError(error);
}
