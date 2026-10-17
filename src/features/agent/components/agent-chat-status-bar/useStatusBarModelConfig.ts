import { useAgentChatState } from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
import { useAgentSessionListState } from '@/context/AgentSessionListContext';
import { useSettings } from '@/hooks/use-settings';
import type { ThinkingEffort } from '@/lib/ai-service/thinking-effort-mapping';
import { getLogger } from '@/lib/logger';
import type { AgentResponse } from '@/models/agent-ipc';
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';

const logger = getLogger('StatusBarModelConfig');

export function useStatusBarModelConfig() {
  const { t } = useTranslation();
  const { session, updateSessionConfig } = useAgentSession();
  const { sessions } = useAgentSessionListState();
  const { workflowStatus } = useAgentChatState();
  const { value: settings, update: updateSettings } = useSettings();

  const [showBatchDialog, setShowBatchDialog] = useState(false);
  const [pendingModelConfig, setPendingModelConfig] = useState<{
    model: string;
    provider: string;
  } | null>(null);
  const [descendantCount, setDescendantCount] = useState(0);

  const canUpdateSessionConfig =
    workflowStatus === 'idle' ||
    workflowStatus === 'paused' ||
    workflowStatus === 'error';

  const handleThinkingEffortChange = useCallback(
    async (effort: ThinkingEffort) => {
      await updateSettings({
        advanced: {
          ...settings.advanced,
          thinkingEffort: effort,
        },
      });
    },
    [settings.advanced, updateSettings],
  );

  const performConfigUpdate = useCallback(
    async (model: string, provider: string, recursive: boolean) => {
      if (!session?.id || !session.assistant) return;
      try {
        const { safeInvoke } = await import('@/lib/backend/core');

        await safeInvoke<AgentResponse>('agent_update_session_config', {
          request: {
            sessionId: session.id,
            model,
            provider,
            assistantId: session.assistant.id,
            recursive,
          },
        });

        updateSessionConfig(model, provider);
        if (workflowStatus === 'error') {
          toast.success(
            t(
              'agent.statusBar.configUpdatedRecoveryHint',
              'Model updated. Retry to recover the session.',
            ),
          );
        }
      } catch (e) {
        logger.error('Failed to update session config', e);
        toast.error(
          t(
            'agent.statusBar.configUpdateError',
            'Failed to update the model configuration.',
          ),
        );
      }
    },
    [session, t, updateSessionConfig, workflowStatus],
  );

  const handleConfigUpdate = useCallback(
    async (model: string, provider: string) => {
      if (!session?.id || !session.assistant || !canUpdateSessionConfig) {
        return;
      }

      logger.info(`Updating session config to ${provider}/${model}`);

      try {
        const { safeInvoke } = await import('@/lib/backend/core');
        const descendantIds = await safeInvoke<string[]>(
          'agent_get_descendant_session_ids',
          {
            sessionId: session.id,
          },
        );

        if (descendantIds && descendantIds.length > 0) {
          setDescendantCount(descendantIds.length);
          setPendingModelConfig({ model, provider });
          setShowBatchDialog(true);
        } else {
          await performConfigUpdate(model, provider, false);
        }
      } catch (err) {
        logger.error('Failed to query descendant session ids', err);
        // Fallback to local sessions array in case of IPC error
        const subSessions = sessions.filter(
          (s) => s.parentSessionId === session.id,
        );
        if (subSessions.length > 0) {
          setDescendantCount(subSessions.length);
          setPendingModelConfig({ model, provider });
          setShowBatchDialog(true);
        } else {
          await performConfigUpdate(model, provider, false);
        }
      }
    },
    [canUpdateSessionConfig, performConfigUpdate, session, sessions],
  );

  const closeBatchDialog = useCallback(() => {
    setShowBatchDialog(false);
    setPendingModelConfig(null);
  }, []);

  const confirmBatchUpdate = useCallback(
    (recursive: boolean) => {
      if (pendingModelConfig) {
        void performConfigUpdate(
          pendingModelConfig.model,
          pendingModelConfig.provider,
          recursive,
        );
      }
    },
    [pendingModelConfig, performConfigUpdate],
  );

  return {
    session,
    thinkingEffort: settings.advanced?.thinkingEffort,
    canUpdateSessionConfig,
    showBatchDialog,
    pendingModelConfig,
    descendantCount,
    handleThinkingEffortChange,
    handleConfigUpdate,
    closeBatchDialog,
    confirmBatchUpdate,
  };
}
