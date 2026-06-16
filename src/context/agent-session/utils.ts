import type { MessageError } from '@/models/chat';
import type { AgentRuntimeError } from '@/models/agent-ipc';
import { getAgentSessionMetadata } from '@/lib/backend/agent-commands';
import { coalesceExecutionModeFlags } from '@/lib/session-metadata';
import { getLogger } from '@/lib/logger';
import type { useAgentSessionState } from './useAgentSessionState';

const logger = getLogger('AgentSessionSync');

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

export async function syncSessionMetadataFromBackend(
  sessionId: string,
  setters: ReturnType<typeof useAgentSessionState>['setters'],
): Promise<void> {
  try {
    const metadata = await getAgentSessionMetadata(sessionId);
    if (!metadata) {
      return;
    }

    const executionMode = coalesceExecutionModeFlags(
      metadata.yoloMode,
      metadata.unsafeMode,
    );

    setters.applyExecutionMode(executionMode.executionMode);
    setters.setSession((previous) =>
      previous
        ? {
            ...previous,
            name: metadata.name,
            status: metadata.status,
            yoloMode: executionMode.yoloMode,
            unsafeMode: executionMode.unsafeMode,
          }
        : previous,
    );
  } catch (error) {
    logger.error('Failed to sync session metadata from backend', error);
  }
}
