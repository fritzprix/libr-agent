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

/**
 * Only a real failure (process exited, initialize returned an error, or the safety
 * cap ran out) toasts. `timed_out` means the session stopped waiting while the server
 * keeps starting in the background, so it is shown as "still starting", not as an error.
 */
function isToastableServerFailure(
  status: SessionRuntimeServerState['status'],
): boolean {
  return status === 'failed';
}

/**
 * Diff runtime server snapshots and return newly failed servers that should toast
 * once.
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

    const kind: McpServerFailureToastKind = isMcpServerTimeoutError(error)
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
