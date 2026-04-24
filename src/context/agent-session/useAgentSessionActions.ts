import { useCallback } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { getMessagesBeforeCursor } from '@/lib/backend/messages';
import { getLogger } from '@/lib/logger';
import type { Message, RustMessage } from '@/models/chat';
import type { AgentResponse, SendUserMessageRequest } from '@/models/agent-ipc';
import type { useAgentSessionState } from './useAgentSessionState';

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

  const toggleYoloMode = useCallback(async () => {
    const newVal = !state.yoloModeEnabled;
    try {
      await safeInvoke<void>('agent_set_yolo_mode', {
        sessionId,
        enabled: newVal,
      });
      setters.setYoloModeEnabled(newVal);
      logger.info(`YOLO mode ${newVal ? 'enabled' : 'disabled'}`);

      if (newVal && state.pendingApprovals.length > 0) {
        logger.info('Auto-approving pending tools due to YOLO toggle', {
          count: state.pendingApprovals.length,
        });
        const approvalsToClear = [...state.pendingApprovals];
        approvalsToClear.forEach((p) => {
          void safeInvoke<AgentResponse>('agent_respond_tool_approval', {
            sessionId,
            toolCallId: p.toolCallId,
            approved: true,
          }).catch((err) => {
            logger.error('Failed to auto-approve tool upon YOLO toggle', err);
          });
          externalActions.clearPendingApproval(sessionId, p.toolCallId);
        });
        setters.setPendingApprovals([]);
        setters.setWorkflowPhase('using_tools');
        await externalActions.acknowledgeSessionAttention();
      }
    } catch (err) {
      logger.error('Failed to toggle YOLO mode on backend', err);
    }
  }, [
    externalActions,
    sessionId,
    setters,
    state.pendingApprovals,
    state.yoloModeEnabled,
  ]);

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
    toggleYoloMode,
    updateSessionConfig,
  };
}
