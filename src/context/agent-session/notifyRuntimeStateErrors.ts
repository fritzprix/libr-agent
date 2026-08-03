import { toast } from 'sonner';
import type { SessionRuntimeState } from '@/models/agent-ipc';

/**
 * Surface session-level MCP init failures via Sonner when runtime phase reaches
 * failed without per-server details. Per-server failures (including timeouts)
 * are handled by `useMcpServerFailureToasts` so one server cannot suppress another.
 */
export function notifyRuntimeStateErrors(
  prevState: SessionRuntimeState,
  nextState: SessionRuntimeState,
): void {
  if (
    prevState.phase !== 'failed' &&
    nextState.phase === 'failed' &&
    nextState.servers.every((server) => server.status !== 'failed') &&
    nextState.initialization.error
  ) {
    toast.error('MCP Server initialization failed', {
      description: nextState.initialization.error,
      duration: 8000,
    });
  }
}
