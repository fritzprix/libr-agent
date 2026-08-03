import type { SessionRuntimeServerState } from '@/models/agent-ipc';

export type McpServerFailureToastKind = 'timeout' | 'failed';

export interface McpServerFailureToast {
  key: string;
  serverName: string;
  transport: SessionRuntimeServerState['transport'];
  kind: McpServerFailureToastKind;
  error: string;
}

export function isMcpServerTimeoutError(error: string): boolean {
  const normalized = error.toLowerCase();
  return (
    normalized.includes('timed out') ||
    normalized.includes('timeout') ||
    normalized.includes('wait budget')
  );
}

function isToastableServerFailure(
  status: SessionRuntimeServerState['status'],
): boolean {
  return status === 'failed' || status === 'timed_out';
}

/**
 * Diff runtime server snapshots and return newly failed/timed-out servers that
 * should toast once.
 */
export function collectNewMcpServerFailures(
  previousKeys: ReadonlySet<string>,
  servers: readonly SessionRuntimeServerState[],
): McpServerFailureToast[] {
  const toasts: McpServerFailureToast[] = [];

  for (const server of servers) {
    if (!isToastableServerFailure(server.status)) {
      continue;
    }

    const error = server.error?.trim() ?? '';
    const key = `${server.transport}:${server.name}:${server.status}:${error || 'unknown'}`;
    if (previousKeys.has(key)) {
      continue;
    }

    const kind: McpServerFailureToastKind =
      server.status === 'timed_out' || isMcpServerTimeoutError(error)
        ? 'timeout'
        : 'failed';

    toasts.push({
      key,
      serverName: server.name,
      transport: server.transport,
      kind,
      error: error || 'Unknown error',
    });
  }

  return toasts;
}
