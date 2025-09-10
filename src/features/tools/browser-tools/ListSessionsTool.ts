import { listBrowserSessions } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { StrictLocalMCPTool } from './types';

const logger = getLogger('ListSessionsTool');

export const listSessionsTool: StrictLocalMCPTool = {
  name: 'listSessions',
  description: 'Lists all active browser sessions.',
  inputSchema: {
    type: 'object',
    properties: {},
    required: [],
  },
  execute: async () => {
    logger.debug('Executing browser_listSessions');
    const sessions = await listBrowserSessions();
    return createMCPTextResponse(
      `Active browser sessions: ${JSON.stringify(sessions)}`,
    );
  },
};
