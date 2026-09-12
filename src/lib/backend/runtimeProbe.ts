import { safeInvoke } from './core';

export interface RuntimeProbeResult {
  node: boolean;
  npx: boolean;
  python: boolean;
  uv: boolean;
}

/**
 * Lightweight PATH probe for MCP runtime binaries (node/npx/python/uv).
 */
export async function probeRuntimeBinaries(): Promise<RuntimeProbeResult> {
  return safeInvoke<RuntimeProbeResult>('probe_runtime_binaries');
}

/** True when stdio MCP servers that launch via `npx` can start. */
export function isNpxRuntimeReady(
  probe: RuntimeProbeResult | null | undefined,
): boolean {
  return Boolean(probe?.npx);
}
