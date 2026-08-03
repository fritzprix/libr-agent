import { AlertCircle, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { isMcpServerTimeoutError } from '@/context/agent-session/mcpServerFailureFeedback';
import type {
  SessionRuntimeInitResult,
  SessionRuntimeServerState,
} from '@/models/agent-ipc';
import { cn } from '@/lib/utils';

interface SessionLoadingOverlayProps {
  label: string;
  initializationStep?: string | null;
  initializationError?: string | null;
  variant: 'blocking' | 'overlay';
  servers?: SessionRuntimeServerState[];
  initResult?: SessionRuntimeInitResult;
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
}: {
  servers: SessionRuntimeServerState[];
}) {
  const { t } = useTranslation();

  if (servers.length === 0) {
    return null;
  }

  return (
    <ul
      className="flex flex-col gap-1 mt-2 w-full max-w-sm"
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

export function SessionLoadingOverlay({
  label,
  initializationStep,
  initializationError,
  variant,
  servers = [],
  initResult,
}: SessionLoadingOverlayProps) {
  const isFailed = Boolean(initializationError) || initResult === 'failed';

  const content = (
    <>
      {!isFailed ? (
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
          {label}
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
          ) : initializationStep ? (
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
