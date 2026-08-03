import { useMemo } from 'react';

import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  buildMcpToolsDiscoveryRevision,
  useAgentTools,
} from '@/hooks/use-agent-tools';

/**
 * Session-scoped agent tools with discovery-revision cache invalidation.
 *
 * Builds the SWR key revision from live MCP runtime servers + proxy ready so
 * callers do not each reimplement `buildMcpToolsDiscoveryRevision`.
 */
export function useSessionAgentTools() {
  const { session, isProxyReady, runtimeState } = useAgentSessionState();

  const discoveryRevision = useMemo(
    () =>
      buildMcpToolsDiscoveryRevision(
        runtimeState.servers.map((server) => ({
          name: server.name,
          status: server.status,
          toolCount: server.toolCount,
        })),
        isProxyReady,
      ),
    [isProxyReady, runtimeState.servers],
  );

  return useAgentTools(session?.id, { discoveryRevision });
}
