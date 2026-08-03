import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import { collectNewMcpServerFailures } from './mcpServerFailureFeedback';

/**
 * Show Sonner feedback when session MCP servers fail or time out during init/resume.
 * One server's failure must not suppress toasts for others.
 */
export function useMcpServerFailureToasts(
  sessionId: string,
  runtimeState: SessionRuntimeState,
): void {
  const { t } = useTranslation();
  const toastedKeysRef = useRef<Set<string>>(new Set());
  const previousSessionIdRef = useRef(sessionId);

  useEffect(() => {
    if (previousSessionIdRef.current !== sessionId) {
      toastedKeysRef.current = new Set();
      previousSessionIdRef.current = sessionId;
    }

    const newFailures = collectNewMcpServerFailures(
      toastedKeysRef.current,
      runtimeState.servers,
    );

    for (const failure of newFailures) {
      toastedKeysRef.current.add(failure.key);

      if (failure.kind === 'timeout') {
        toast.error(
          t('agent.statusBar.mcpServerTimeout', {
            serverName: failure.serverName,
            transport: failure.transport,
          }),
          {
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
