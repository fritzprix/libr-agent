import { listBrowserSessions } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { LocalMCPTool } from './types';

const logger = getLogger('ListSessionsTool');

export const listSessionsTool: LocalMCPTool = {
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
    return `Active browser sessions: ${JSON.stringify(sessions, null, 2)}`;
  },
};
