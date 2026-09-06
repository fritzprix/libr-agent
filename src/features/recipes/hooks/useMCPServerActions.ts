import { useContext, useMemo } from 'react';
import { MCPServerRegistryContext } from '@/context/MCPServerRegistryContext';
import { upsertMCPServer } from '@/lib/backend/mcp-server-config';
import type { MCPServerEntity } from '@/models/chat';

export function useMCPServerActions() {
  const registry = useContext(MCPServerRegistryContext);

  return useMemo(
    () => ({
      saveServer: async (server: MCPServerEntity): Promise<MCPServerEntity> => {
        if (registry?.saveServer) {
          return await registry.saveServer(server);
        }
        return await upsertMCPServer(server);
      },
    }),
    [registry],
  );
}
