import { useCallback } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { getMessagesBeforeCursor } from '@/lib/backend/messages';
import { getLogger } from '@/lib/logger';
import type { Message, RustMessage } from '@/models/chat';
import type { AgentResponse, SendUserMessageRequest } from '@/models/agent-ipc';
import type { useAgentSessionState } from './useAgentSessionState';
import type { ExecutionMode } from './types';

const logger = getLogger('AgentSessionActions');

export function useAgentSessionActionsLogic(
  sessionId: string,
  stateProps: ReturnType<typeof useAgentSessionState>,
  externalActions: {
    acknowledgeSessionAttention: (viewedAt?: Date) => Promise<void>;
    clearPendingApproval: (sessionId: string, toolCallId: string) => void;
  },
) {
  const { state, setters } = stateProps;
  const currentExecutionMode: ExecutionMode = state.unsafeModeEnabled
    ? 'unsafe'
    : state.yoloModeEnabled
      ? 'yolo'
      : 'normal';

  const applyExecutionModeLocally = useCallback(
    (mode: ExecutionMode) => {
      setters.setYoloModeEnabled(mode === 'yolo');
      setters.setUnsafeModeEnabled(mode === 'unsafe');
      setters.setSession((previous) =>
        previous
          ? {
              ...previous,
              yoloMode: mode === 'yolo',
              unsafeMode: mode === 'unsafe',
            }
          : previous,
      );
    },
    [setters],
  );

  const sendMessage = useCallback(
    async (content: string) => {
      if (!state.session) {
        throw new Error('No active session initialized');
      }

      try {
        const messageId = `msg_${Date.now()}`;
        const now = new Date();
        const message: Message = {
          id: messageId,
          sessionId: state.session.id,
          threadId: state.session.id,
          role: 'user',
          content: [{ type: 'text', text: content }],
          createdAt: now,
          updatedAt: now,
        };

        const rustMessage: RustMessage = {
          id: message.id,
          sessionId: message.sessionId,
          role: message.role,
          content: message.content,
          toolCalls: message.tool_calls,
          toolCallId: message.tool_call_id,
          isStreaming: message.isStreaming,
          thinking: message.thinking,
          thinkingSignature: message.thinkingSignature,
          thinkingTime: message.thinkingTime,
          assistantId: message.assistantId,
          attachments: message.attachments,
          toolUse: message.tool_use,
          createdAt: now.getTime(),
          updatedAt: now.getTime(),
          source: message.source,
          error: message.error,
          metadata: message.metadata,
        };

        const request: SendUserMessageRequest = {
          sessionId: state.session.id,
          message: rustMessage,
        };

        await safeInvoke<AgentResponse>('agent_send_message', { request });
        await externalActions.acknowledgeSessionAttention(now);
      } catch (err) {
        logger.error('Failed to send message', err);
        throw err;
      }
    },
    [externalActions, state.session],
  );

  const stopSession = useCallback(async () => {
    if (!state.session) return;
    try {
      await safeInvoke<AgentResponse>('agent_terminate_workflow', {
        sessionId: state.session.id,
      });
    } catch (err) {
      logger.error('Failed to stop session', err);
    }
  }, [state.session]);

  const loadOlderMessages = useCallback(async () => {
    if (
      !state.session ||
      state.isLoadingOlderMessages ||
      !state.hasOlderMessages ||
      !state.oldestMessageCursor
    ) {
      return;
    }

    try {
      setters.setIsLoadingOlderMessages(true);

      const olderSlice = await getMessagesBeforeCursor(
        state.session.id,
        state.oldestMessageCursor,
        50,
      );

      setters.prependMessages(olderSlice.items);
      setters.setHasOlderMessages(olderSlice.hasMoreBefore);
      setters.setOldestMessageCursor(olderSlice.oldestCursor ?? null);
    } catch (err) {
      logger.error('Failed to load older messages', err);
      throw err;
    } finally {
      setters.setIsLoadingOlderMessages(false);
    }
  }, [
    setters,
    state.hasOlderMessages,
    state.isLoadingOlderMessages,
    state.oldestMessageCursor,
    state.session,
  ]);

  const resumeSession = useCallback(async () => {
    if (!state.session) return;
    try {
      await safeInvoke<AgentResponse>('agent_resume_workflow', {
        sessionId: state.session.id,
      });
      await externalActions.acknowledgeSessionAttention();
    } catch (err) {
      logger.error('Failed to resume session', err);
      throw err;
    }
  }, [externalActions, state.session]);

  const respondToToolApproval = useCallback(
    async (toolCallId: string, approved: boolean) => {
      if (!state.session) return;
      try {
        await safeInvoke<AgentResponse>('agent_respond_tool_approval', {
          sessionId: state.session.id,
          toolCallId,
          approved,
        });

        setters.setPendingApprovals((prev) =>
          prev.filter((p) => p.toolCallId !== toolCallId),
        );
        externalActions.clearPendingApproval(state.session.id, toolCallId);
        await externalActions.acknowledgeSessionAttention();
      } catch (err) {
        logger.error('Failed to respond to tool approval', err);
        throw err;
      }
    },
    [externalActions, setters, state.session],
  );

  const setExecutionMode = useCallback(
    async (mode: ExecutionMode) => {
      if (mode === currentExecutionMode) {
        return;
      }

      try {
        await safeInvoke<void>('agent_set_execution_mode', {
          sessionId,
          mode,
        });

        applyExecutionModeLocally(mode);
        logger.info(`Execution mode set to ${mode}`);

        if (mode !== 'normal' && state.pendingApprovals.length > 0) {
          logger.info(
            'Backend will reconcile pending approvals after execution mode change',
            {
              mode,
              count: state.pendingApprovals.length,
            },
          );
          await externalActions.acknowledgeSessionAttention();
        }
      } catch (err) {
        logger.error('Failed to set execution mode on backend', err);
      }
    },
    [
      applyExecutionModeLocally,
      currentExecutionMode,
      externalActions,
      sessionId,
      state.pendingApprovals.length,
    ],
  );

  const toggleYoloMode = useCallback(async () => {
    await setExecutionMode(currentExecutionMode === 'yolo' ? 'normal' : 'yolo');
  }, [currentExecutionMode, setExecutionMode]);

  const toggleUnsafeMode = useCallback(async () => {
    await setExecutionMode(
      currentExecutionMode === 'unsafe' ? 'normal' : 'unsafe',
    );
  }, [currentExecutionMode, setExecutionMode]);

  const updateSessionConfig = useCallback(
    (model: string, provider: string) => {
      setters.setSession((prev) =>
        prev ? { ...prev, model, provider } : null,
      );
    },
    [setters],
  );

  return {
    sendMessage,
    stopSession,
    loadOlderMessages,
    resumeSession,
    respondToToolApproval,
    setExecutionMode,
    toggleYoloMode,
    toggleUnsafeMode,
    updateSessionConfig,
  };
}
