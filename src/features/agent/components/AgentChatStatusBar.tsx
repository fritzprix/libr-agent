import {
  useAgentChatActions,
  useAgentChatState,
} from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
import { useLLMService } from '@/context/LLMServiceContext';
import { useIsMobile } from '@/hooks/use-mobile';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  StatusBarExecutionMode,
  StatusBarModelControls,
  StatusBarTokenMetrics,
  StatusBarToolsControl,
  StatusBarWorkflowBanner,
} from './agent-chat-status-bar';

const logger = getLogger('AgentChatStatusBar');

export function AgentChatStatusBar() {
  const { t } = useTranslation();
  const isCompactStatusBar = useIsMobile(640);
  const {
    session,
    executionMode,
    setExecutionMode,
    preflightTokenMetrics,
    pendingApprovals,
  } = useAgentSession();
  const { workflowStatus, error, llmError } = useAgentChatState();
  const { retryMessage, resume } = useAgentChatActions();
  const { isCompacting, isAwaitingCompact } = useLLMService();

  const [isRetrying, setIsRetrying] = useState(false);
  const [isResuming, setIsResuming] = useState(false);

  const compacting = session?.id ? isCompacting(session.id) : false;
  const awaitingCompact = session?.id ? isAwaitingCompact(session.id) : false;

  const handleRetry = async () => {
    if (isRetrying) return;
    setIsRetrying(true);
    try {
      await retryMessage();
    } catch (err) {
      logger.error('Failed to retry message:', err);
      toast.error(t('agent.statusBar.retryError'));
    } finally {
      setIsRetrying(false);
    }
  };

  const handleResume = async () => {
    if (isResuming) return;
    setIsResuming(true);
    try {
      await resume();
    } catch (err) {
      logger.error('Failed to resume session:', err);
      toast.error(t('agent.statusBar.resumeError'));
    } finally {
      setIsResuming(false);
    }
  };

  const showWorkflowBanner =
    Boolean(error || llmError) ||
    awaitingCompact ||
    compacting ||
    pendingApprovals.length > 0 ||
    workflowStatus !== 'idle';

  return (
    <>
      <StatusBarWorkflowBanner
        workflowStatus={workflowStatus}
        error={error}
        llmError={llmError}
        awaitingCompact={awaitingCompact}
        compacting={compacting}
        hasPendingApprovals={pendingApprovals.length > 0}
        isRetrying={isRetrying}
        isResuming={isResuming}
        onRetry={() => {
          void handleRetry();
        }}
        onResume={() => {
          void handleResume();
        }}
      />

      <div
        className={cn(
          'border-b px-4 py-1.5',
          showWorkflowBanner && 'border-t border-border/50',
        )}
      >
        <div className="flex flex-wrap items-center gap-2 md:gap-3">
          <StatusBarModelControls />

          <div className="flex min-w-0 flex-wrap items-center gap-2 md:ml-auto md:justify-end">
            <StatusBarExecutionMode
              executionMode={executionMode}
              onExecutionModeChange={(mode) => void setExecutionMode(mode)}
            />

            <StatusBarTokenMetrics
              sessionId={session?.id}
              preflightTokenMetrics={preflightTokenMetrics}
              compacting={compacting}
              awaitingCompact={awaitingCompact}
              compact={isCompactStatusBar}
            />

            <StatusBarToolsControl />
          </div>
        </div>
      </div>
    </>
  );
}
