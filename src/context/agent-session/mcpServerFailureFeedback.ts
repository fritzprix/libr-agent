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
 * Diff runtime server snapshots and return newly failed servers that should toast once.
 */
export function collectNewMcpServerFailures(
  previousKeys: ReadonlySet<string>,
  servers: readonly SessionRuntimeServerState[],
): McpServerFailureToast[] {
  const toasts: McpServerFailureToast[] = [];

  for (const server of servers) {
    if (server.status !== 'failed') {
      continue;
    }

    const error = server.error?.trim() ?? '';
    const key = `${server.transport}:${server.name}:${error || 'failed'}`;
    if (previousKeys.has(key)) {
      continue;
    }

    toasts.push({
      key,
      serverName: server.name,
      transport: server.transport,
      kind: isMcpServerTimeoutError(error) ? 'timeout' : 'failed',
      error: error || 'Unknown error',
    });
  }

  return toasts;
}
