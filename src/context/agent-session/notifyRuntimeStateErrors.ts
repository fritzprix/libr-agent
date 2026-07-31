import { toast } from 'sonner';
import type { SessionRuntimeState } from '@/models/agent-ipc';

/**
 * Surface MCP server load failures via Sonner when runtime phase reaches
 * degraded/failed. Dedupes per-server so repeated events do not spam.
 */
export function notifyRuntimeStateErrors(
  prevState: SessionRuntimeState,
  nextState: SessionRuntimeState,
): void {
  if (nextState.phase === 'failed' || nextState.phase === 'degraded') {
    const failedServers = nextState.servers.filter(
      (s) => s.status === 'failed',
    );
    if (failedServers.length > 0) {
      failedServers.forEach((server) => {
        const prevServerState = prevState.servers.find(
          (s) => s.name === server.name,
        );
        if (!prevServerState || prevServerState.status !== 'failed') {
          toast.error(`MCP Server '${server.name}' failed to load`, {
            description:
              server.error ??
              nextState.initialization.error ??
              'Initialization failed',
            duration: 8000,
          });
        }
      });
    } else if (
      prevState.phase !== 'failed' &&
      nextState.phase === 'failed' &&
      nextState.initialization.error
    ) {
      toast.error('MCP Server initialization failed', {
        description: nextState.initialization.error,
        duration: 8000,
      });
    }
  }
}
