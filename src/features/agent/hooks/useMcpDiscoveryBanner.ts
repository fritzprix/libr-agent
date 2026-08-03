import { useEffect, useState } from 'react';

import type {
  SessionRuntimeInitResult,
  SessionRuntimePhase,
  SessionRuntimeServerState,
} from '@/models/agent-ipc';

const SUCCESS_RESULT_HOLD_MS = 2500;

export type McpDiscoveryBannerKind = 'loading' | 'result' | null;

interface UseMcpDiscoveryBannerArgs {
  hasSession: boolean;
  isProxyReady: boolean;
  phase: SessionRuntimePhase;
  initResult: SessionRuntimeInitResult;
  servers: readonly SessionRuntimeServerState[];
  sessionId?: string;
}

interface UseMcpDiscoveryBannerResult {
  bannerKind: McpDiscoveryBannerKind;
  dismissResultBanner: () => void;
}

/**
 * Controls MCP discovery banner visibility:
 * - loading while proxy is not ready (and phase is not failed)
 * - result hold after discovery when servers were configured
 */
export function useMcpDiscoveryBanner({
  hasSession,
  isProxyReady,
  phase,
  initResult,
  servers,
  sessionId,
}: UseMcpDiscoveryBannerArgs): UseMcpDiscoveryBannerResult {
  const [dismissedResult, setDismissedResult] = useState(false);

  useEffect(() => {
    setDismissedResult(false);
  }, [sessionId]);

  const hasServers = servers.length > 0;
  const isTerminalPhase =
    phase === 'ready' || phase === 'degraded' || phase === 'failed';
  const discoveryFinished =
    isProxyReady || phase === 'failed' || initResult !== 'pending';

  const showLoading = hasSession && !isProxyReady && phase !== 'failed';

  const showResult =
    hasSession &&
    hasServers &&
    discoveryFinished &&
    isTerminalPhase &&
    !dismissedResult &&
    !showLoading &&
    (initResult === 'success' ||
      initResult === 'partial' ||
      initResult === 'failed' ||
      phase === 'degraded' ||
      phase === 'failed');

  useEffect(() => {
    if (!showResult || initResult !== 'success') {
      return;
    }

    const timer = window.setTimeout(() => {
      setDismissedResult(true);
    }, SUCCESS_RESULT_HOLD_MS);

    return () => {
      window.clearTimeout(timer);
    };
  }, [showResult, initResult, sessionId]);

  let bannerKind: McpDiscoveryBannerKind = null;
  if (showLoading) {
    bannerKind = 'loading';
  } else if (showResult) {
    bannerKind = 'result';
  }

  return {
    bannerKind,
    dismissResultBanner: () => setDismissedResult(true),
  };
}
