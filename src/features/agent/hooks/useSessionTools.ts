import { useState, useEffect } from 'react';
import type { MCPTool } from '@/lib/mcp';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useSessionTools');

export function useSessionTools(sessionId: string | undefined): {
  tools: MCPTool[];
} {
  const [tools, setTools] = useState<MCPTool[]>([]);

  useEffect(() => {
    if (!sessionId) {
      setTools([]);
      return;
    }
    getAgentAvailableTools(sessionId)
      .then(setTools)
      .catch((err) => {
        logger.warn('Failed to fetch session tools', err);
        setTools([]);
      });
  }, [sessionId]);

  return { tools };
}
