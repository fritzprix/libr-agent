import { useCallback, useEffect, useState } from 'react';
import {
  isNpxRuntimeReady,
  probeRuntimeBinaries,
  type RuntimeProbeResult,
} from '@/lib/backend/runtimeProbe';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useRuntimeReadiness');

export interface UseRuntimeReadinessOptions {
  /** When false, skips probing until re-enabled. Defaults to true. */
  enabled?: boolean;
}

export interface RuntimeReadiness {
  /** Latest probe result; null while loading or if the probe failed. */
  probe: RuntimeProbeResult | null;
  loading: boolean;
  error: string | null;
  /** True when `npx` is available for stdio MCP servers. */
  isNpxReady: boolean;
  refresh: () => Promise<void>;
}

/**
 * Probes host PATH for MCP runtime binaries (node/npx/python/uv).
 */
export function useRuntimeReadiness(
  options: UseRuntimeReadinessOptions = {},
): RuntimeReadiness {
  const { enabled = true } = options;
  const [probe, setProbe] = useState<RuntimeProbeResult | null>(null);
  const [loading, setLoading] = useState(enabled);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await probeRuntimeBinaries();
      setProbe(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logger.warn('Runtime probe failed', err);
      setError(message);
      setProbe(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      setLoading(false);
      return;
    }
    void refresh();
  }, [enabled, refresh]);

  return {
    probe,
    loading,
    error,
    isNpxReady: isNpxRuntimeReady(probe),
    refresh,
  };
}
