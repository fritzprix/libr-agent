import { Button } from '@/components/ui/button';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import type { MessageError } from '@/models/chat';
import {
  AlertCircle,
  Clock,
  DatabaseZap,
  Info,
  Loader2,
  Pause,
  Play,
  RefreshCw,
  Shield,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

export interface StatusBarWorkflowBannerProps {
  workflowStatus:
    | 'idle'
    | 'busy'
    | 'paused'
    | 'error'
    | 'queued'
    | 'provisioning';
  error: MessageError | null;
  llmError: MessageError | null;
  awaitingCompact: boolean;
  compacting: boolean;
  hasPendingApprovals: boolean;
  isRetrying: boolean;
  isResuming: boolean;
  onRetry: () => void;
  onResume: () => void;
}

export function StatusBarWorkflowBanner({
  workflowStatus,
  error,
  llmError,
  awaitingCompact,
  compacting,
  hasPendingApprovals,
  isRetrying,
  isResuming,
  onRetry,
  onResume,
}: StatusBarWorkflowBannerProps) {
  const { t } = useTranslation();

  const getStatusConfig = () => {
    if (error || llmError) {
      const activeError = error ?? llmError;
      return {
        icon: <AlertCircle className="w-4 h-4" />,
        text: t('agent.statusBar.statusError', {
          error: activeError?.displayMessage ?? '',
        }),
        className: 'bg-destructive/10 border-destructive/20 text-destructive',
        showRetry: true,
        showResume: false,
      };
    }

    // Compact-specific states take priority over generic 'busy'
    if (awaitingCompact) {
      return {
        icon: <Loader2 className="w-4 h-4 animate-spin" />,
        text: t('agent.statusBar.statusAwaitingCompact'),
        className:
          'bg-blue-500/10 border-blue-500/20 text-blue-700 dark:text-blue-400',
        showRetry: false,
        showResume: false,
      };
    }

    if (compacting) {
      return {
        icon: (
          <DatabaseZap className="w-4 h-4 animate-pulse text-muted-foreground" />
        ),
        text: t('agent.statusBar.statusCompacting'),
        className: 'bg-secondary/50 border-border text-muted-foreground',
        showRetry: false,
        showResume: false,
      };
    }

    if (hasPendingApprovals) {
      return {
        icon: <Shield className="w-4 h-4" />,
        text: t(
          'agent.statusBar.statusAwaitingApproval',
          'Awaiting approval — review pending tool actions.',
        ),
        className:
          'bg-amber-500/10 border-amber-500/20 text-amber-800 dark:text-amber-300',
        showRetry: false,
        showResume: false,
      };
    }

    switch (workflowStatus) {
      case 'idle':
        return {
          icon: <Info className="w-4 h-4" />,
          text: t('agent.statusBar.statusIdle'),
          className: 'bg-secondary/50 border-border text-foreground',
          showRetry: false,
          showResume: false,
        };
      case 'busy':
        return {
          icon: <Loader2 className="w-4 h-4 animate-spin" />,
          text: t('agent.statusBar.statusBusy'),
          className: 'bg-warning/10 border-warning/20 text-warning-foreground',
          showRetry: false,
          showResume: false,
        };
      case 'queued':
        return {
          icon: <Clock className="w-4 h-4 text-warning animate-pulse" />,
          text: t(
            'agent.statusBar.statusQueued',
            'Queued (Waiting for available agent slot...)',
          ),
          className: 'bg-warning/10 border-warning/20 text-warning-foreground',
          showRetry: false,
          showResume: false,
        };
      case 'paused':
        return {
          icon: <Pause className="w-4 h-4" />,
          text: t('agent.statusBar.statusPaused'),
          className: 'bg-secondary/50 border-border text-foreground',
          showRetry: false,
          showResume: true,
        };
      case 'error':
        return {
          icon: <AlertCircle className="w-4 h-4" />,
          text: t('agent.statusBar.statusWorkflowError'),
          className: 'bg-destructive/10 border-destructive/20 text-destructive',
          showRetry: true,
          showResume: false,
        };
      default:
        return {
          icon: <Info className="w-4 h-4" />,
          text: t('agent.statusBar.statusUnknown', { status: workflowStatus }),
          className: 'bg-muted/50 border-border text-muted-foreground',
          showRetry: false,
          showResume: false,
        };
    }
  };

  const showWorkflowBanner =
    Boolean(error || llmError) ||
    awaitingCompact ||
    compacting ||
    hasPendingApprovals ||
    workflowStatus !== 'idle';

  if (!showWorkflowBanner) {
    return null;
  }

  const config = getStatusConfig();

  return (
    <div
      data-testid="agent-workflow-status"
      className={`flex items-center justify-between px-4 py-1.5 ${config.className}`}
    >
      <div className="flex items-center gap-2">
        {config.icon}
        <span className="text-sm">{config.text}</span>
      </div>
      {config.showRetry ? (
        <Button
          size="sm"
          variant="outline"
          onClick={onRetry}
          className="h-7"
          disabled={isRetrying}
        >
          {isRetrying ? (
            <LoadingSpinner size="sm" className="mr-1" />
          ) : (
            <RefreshCw className="w-3 h-3 mr-1" />
          )}
          {isRetrying
            ? t('agent.statusBar.retrying')
            : t('agent.statusBar.retry')}
        </Button>
      ) : null}
      {config.showResume ? (
        <Button
          size="sm"
          variant="outline"
          onClick={onResume}
          className="h-7"
          disabled={isResuming}
        >
          {isResuming ? (
            <LoadingSpinner size="sm" className="mr-1" />
          ) : (
            <Play className="w-3 h-3 mr-1" />
          )}
          {isResuming
            ? t('agent.statusBar.resuming')
            : t('agent.statusBar.continue')}
        </Button>
      ) : null}
    </div>
  );
}
