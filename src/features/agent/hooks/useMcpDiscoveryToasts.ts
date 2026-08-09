import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';

import type {
  SessionRuntimeInitResult,
  SessionRuntimePhase,
  SessionRuntimeServerState,
} from '@/models/agent-ipc';

const SUCCESS_TOAST_MS = 2500;
const PARTIAL_TOAST_MS = 5000;

interface UseMcpDiscoveryToastsArgs {
  hasSession: boolean;
  isProxyReady: boolean;
  phase: SessionRuntimePhase;
  initResult: SessionRuntimeInitResult;
  servers: readonly SessionRuntimeServerState[];
  sessionId?: string;
  currentStep?: string | null;
}

function discoveryToastId(sessionId: string): string {
  return `mcp-discovery:${sessionId}`;
}

/**
 * Surfaces MCP discovery progress via Sonner instead of a chat-top banner.
 * Per-server failed/timed_out toasts remain in useMcpServerFailureToasts.
 */
export function useMcpDiscoveryToasts({
  hasSession,
  isProxyReady,
  phase,
  initResult,
  servers,
  sessionId,
  currentStep,
}: UseMcpDiscoveryToastsArgs): void {
  const { t } = useTranslation();
  const resultKeyBySessionRef = useRef<Map<string, string>>(new Map());

  const showLoading = hasSession && !isProxyReady && phase !== 'failed';
  const discoveryFinished =
    isProxyReady || phase === 'failed' || initResult !== 'pending';
  const isTerminalPhase =
    phase === 'ready' || phase === 'degraded' || phase === 'failed';
  const hasServers = servers.length > 0;

  useEffect(() => {
    if (!sessionId || !hasSession) {
      return;
    }

    const id = discoveryToastId(sessionId);

    if (showLoading) {
      toast.loading(currentStep || t('agent.statusBar.loadingTools'), {
        id,
      });
      return;
    }

    // Always clear the loading toast once proxy init is no longer in progress.
    // Builtin-only sessions (servers=[]) never show a result toast, so dismiss is
    // required here. Sessions with external MCP reuse the same id for success/warn/error.
    toast.dismiss(id);
  }, [sessionId, hasSession, showLoading, currentStep, t]);

  useEffect(() => {
    if (!sessionId || !hasSession || showLoading) {
      return;
    }
    if (!discoveryFinished || !isTerminalPhase || !hasServers) {
      return;
    }

    const resultKey = `${sessionId}:${initResult}:${phase}`;
    if (resultKeyBySessionRef.current.get(sessionId) === resultKey) {
      return;
    }
    resultKeyBySessionRef.current.set(sessionId, resultKey);

    const id = discoveryToastId(sessionId);

    if (initResult === 'success') {
      toast.success(t('agent.statusBar.mcpResultSuccess'), {
        id,
        duration: SUCCESS_TOAST_MS,
      });
      return;
    }

    if (initResult === 'partial' || phase === 'degraded') {
      toast.warning(t('agent.statusBar.mcpResultPartial'), {
        id,
        duration: PARTIAL_TOAST_MS,
      });
      return;
    }

    if (initResult === 'failed' || phase === 'failed') {
      const hasPerServerFeedback = servers.some(
        (server) => server.status === 'failed' || server.status === 'timed_out',
      );
      if (!hasPerServerFeedback) {
        toast.error(t('agent.statusBar.mcpResultFailed'), {
          id,
          duration: 8000,
        });
      }
    }
  }, [
    sessionId,
    hasSession,
    showLoading,
    discoveryFinished,
    isTerminalPhase,
    hasServers,
    initResult,
    phase,
    servers,
    t,
  ]);

  useEffect(() => {
    if (!sessionId) {
      return;
    }
    const id = discoveryToastId(sessionId);
    return () => {
      toast.dismiss(id);
    };
  }, [sessionId]);
}
