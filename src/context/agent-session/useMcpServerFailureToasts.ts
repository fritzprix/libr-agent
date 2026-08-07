import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import { collectNewMcpServerFailures } from './mcpServerFailureFeedback';

export function mcpServerFailureToastId(
  sessionId: string,
  failureKey: string,
): string {
  return `mcp-failure:${sessionId}:${failureKey}`;
}

/**
 * Show Sonner feedback when session MCP servers fail or time out during init/resume.
 * One server's failure must not suppress toasts for others.
 */
export function useMcpServerFailureToasts(
  sessionId: string,
  runtimeState: SessionRuntimeState,
): void {
  const { t } = useTranslation();
  const toastedKeysBySessionRef = useRef<Map<string, Set<string>>>(new Map());
  const activeToastIdsRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    return () => {
      for (const toastId of activeToastIdsRef.current) {
        toast.dismiss(toastId);
      }
      activeToastIdsRef.current.clear();
    };
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId) {
      return;
    }

    if (!toastedKeysBySessionRef.current.has(sessionId)) {
      toastedKeysBySessionRef.current.set(sessionId, new Set());
    }
    const sessionToastedKeys = toastedKeysBySessionRef.current.get(sessionId)!;

    const newFailures = collectNewMcpServerFailures(
      sessionToastedKeys,
      runtimeState.servers,
    );

    for (const failure of newFailures) {
      sessionToastedKeys.add(failure.key);
      const toastId = mcpServerFailureToastId(sessionId, failure.key);
      activeToastIdsRef.current.add(toastId);

      if (failure.kind === 'timeout') {
        toast.error(
          t('agent.statusBar.mcpServerTimeout', {
            serverName: failure.serverName,
            transport: failure.transport,
          }),
          {
            id: toastId,
            description: t('agent.statusBar.mcpServerTimeoutDescription', {
              error: failure.error,
            }),
            duration: 8000,
          },
        );
      } else {
        toast.error(
          t('agent.statusBar.mcpServerFailed', {
            serverName: failure.serverName,
            transport: failure.transport,
          }),
          {
            id: toastId,
            description: t('agent.statusBar.mcpServerFailedDescription', {
              error: failure.error,
            }),
            duration: 8000,
          },
        );
      }
    }
  }, [runtimeState.servers, sessionId, t]);
}
