import { AlertCircle, Check, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { isMcpServerTimeoutError } from '@/context/agent-session/mcpServerFailureFeedback';
import type {
  SessionRuntimeInitResult,
  SessionRuntimeServerState,
} from '@/models/agent-ipc';
import { cn } from '@/lib/utils';

export type SessionLoadingOverlayMode = 'loading' | 'result';

interface SessionLoadingOverlayProps {
  label: string;
  initializationStep?: string | null;
  initializationError?: string | null;
  variant: 'blocking' | 'overlay' | 'banner';
  servers?: SessionRuntimeServerState[];
  initResult?: SessionRuntimeInitResult;
  mode?: SessionLoadingOverlayMode;
  onDismiss?: () => void;
}

function serverStatusLabel(
  server: SessionRuntimeServerState,
  t: (key: string, options?: Record<string, string | number>) => string,
): string {
  switch (server.status) {
    case 'ready':
      return t('agent.statusBar.mcpServerReadyTools', {
        count: server.toolCount,
      });
    case 'connecting':
      return t('agent.statusBar.mcpServerConnecting');
    case 'discovering_tools':
      return t('agent.statusBar.mcpServerDiscovering');
    case 'timed_out':
      return t('agent.statusBar.mcpServerTimedOut');
    case 'failed': {
      const error = server.error?.trim() ?? '';
      if (error && isMcpServerTimeoutError(error)) {
        return t('agent.statusBar.mcpServerTimedOut');
      }
      return t('agent.statusBar.mcpServerFailedShort');
    }
    default:
      return server.transport;
  }
}

function ServerStatusIcon({
  server,
}: {
  server: SessionRuntimeServerState;
}) {
  if (server.status === 'ready') {
    return <Check className="size-3.5 shrink-0 text-emerald-600 dark:text-emerald-400" />;
  }
  if (server.status === 'failed' || server.status === 'timed_out') {
    return <AlertCircle className="size-3.5 shrink-0 text-destructive" />;
  }
  return (
    <LoadingSpinner
      size="sm"
      className="border-2 text-amber-600 dark:text-amber-400 shrink-0"
    />
  );
}

function McpServerStatusList({
  servers,
  compact,
}: {
  servers: SessionRuntimeServerState[];
  compact?: boolean;
}) {
  const { t } = useTranslation();

  if (servers.length === 0) {
    return null;
  }

  return (
    <ul
      className={cn(
        'flex flex-col gap-1',
        compact ? 'mt-1 max-w-full' : 'mt-2 w-full max-w-sm',
      )}
      data-testid="mcp-server-status-list"
    >
      {servers.map((server) => (
        <li
          key={`${server.transport}:${server.name}`}
          className={cn(
            'flex items-center gap-2 text-xs',
            server.status === 'failed' || server.status === 'timed_out'
              ? 'text-destructive/90'
              : server.status === 'ready'
                ? 'text-emerald-700 dark:text-emerald-300'
                : 'text-muted-foreground',
          )}
        >
          <ServerStatusIcon server={server} />
          <span className="font-medium truncate">{server.name}</span>
          <span className="opacity-70 shrink-0">{server.transport}</span>
          <span className="opacity-80 truncate">
            {serverStatusLabel(server, t)}
          </span>
        </li>
      ))}
    </ul>
  );
}

function resultHeadline(
  initResult: SessionRuntimeInitResult | undefined,
  t: (key: string) => string,
  fallback: string,
): string {
  switch (initResult) {
    case 'success':
      return t('agent.statusBar.mcpResultSuccess');
    case 'partial':
      return t('agent.statusBar.mcpResultPartial');
    case 'failed':
      return t('agent.statusBar.mcpResultFailed');
    default:
      return fallback;
  }
}

export function SessionLoadingOverlay({
  label,
  initializationStep,
  initializationError,
  variant,
  servers = [],
  initResult,
  mode = 'loading',
  onDismiss,
}: SessionLoadingOverlayProps) {
  const { t } = useTranslation();
  const isFailed = Boolean(initializationError) || initResult === 'failed';
  const isResultMode = mode === 'result';
  const headline = isResultMode
    ? resultHeadline(initResult, t, initializationStep ?? label)
    : (initializationError ?? initializationStep ?? label);
  const canDismiss = isResultMode && Boolean(onDismiss);

  if (variant === 'banner') {
    return (
      <div
        role="status"
        aria-live="polite"
        className={cn(
          'absolute top-0 left-0 right-0 z-20 border-b px-4 py-2 text-xs backdrop-blur-sm animate-in fade-in slide-in-from-top-1 duration-200',
          isFailed
            ? 'border-destructive/30 bg-destructive/10 text-destructive dark:border-destructive/40 dark:bg-destructive/20'
            : initResult === 'partial'
              ? 'border-amber-200/50 bg-amber-50/90 text-amber-800 dark:border-amber-900/40 dark:bg-amber-950/80 dark:text-amber-200'
              : isResultMode && initResult === 'success'
                ? 'border-emerald-200/50 bg-emerald-50/90 text-emerald-800 dark:border-emerald-900/40 dark:bg-emerald-950/80 dark:text-emerald-200'
                : 'border-amber-200/50 bg-amber-50/90 text-amber-800 dark:border-amber-900/40 dark:bg-amber-950/80 dark:text-amber-200',
        )}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <div className="flex items-center gap-2">
              {!isResultMode && !isFailed ? (
                <LoadingSpinner
                  size="sm"
                  className="border-2 text-amber-600 dark:text-amber-400 shrink-0"
                  label={label}
                />
              ) : null}
              {isResultMode && initResult === 'success' ? (
                <Check className="size-3.5 shrink-0" />
              ) : null}
              {(isFailed || initResult === 'partial') && isResultMode ? (
                <AlertCircle className="size-3.5 shrink-0" />
              ) : null}
              <span className="font-medium">
                {isResultMode
                  ? headline
                  : (initializationError ?? initializationStep ?? label)}
              </span>
            </div>
            <McpServerStatusList servers={servers} compact />
          </div>
          {canDismiss ? (
            <button
              type="button"
              onClick={onDismiss}
              className="shrink-0 rounded p-0.5 opacity-70 hover:opacity-100"
              aria-label={t('agent.statusBar.mcpResultDismiss')}
            >
              <X className="size-3.5" />
            </button>
          ) : null}
        </div>
      </div>
    );
  }

  const content = (
    <>
      {!isFailed && !isResultMode ? (
        <LoadingSpinner size="lg" className="border-4" label={label} />
      ) : null}

      <div className="flex flex-col items-center gap-1">
        <div
          className={
            isFailed
              ? 'text-destructive font-medium'
              : variant === 'blocking'
                ? 'text-muted-foreground font-medium animate-pulse'
                : 'text-muted-foreground font-medium'
          }
          aria-hidden="true"
        >
          {isResultMode ? headline : label}
        </div>

        <div
          className={
            isFailed
              ? 'text-xs text-destructive/80 max-w-sm text-center'
              : 'text-xs text-muted-foreground/70'
          }
        >
          {isFailed && initializationError ? (
            <span>{initializationError}</span>
          ) : !isResultMode && initializationStep ? (
            <span className="animate-in fade-in slide-in-from-bottom-1 duration-300">
              {initializationStep}
            </span>
          ) : null}
        </div>

        <McpServerStatusList servers={servers} />
      </div>
    </>
  );

  if (variant === 'blocking') {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <div className="flex flex-col items-center gap-3">{content}</div>
      </div>
    );
  }

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/60 backdrop-blur-[1px]">
      <div className="flex flex-col items-center gap-3 rounded-xl border border-border/60 bg-background/90 px-6 py-5 shadow-lg">
        {content}
      </div>
    </div>
  );
}
