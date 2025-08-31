import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('GetPageTitleTool');

export const getPageTitleTool: BrowserLocalMCPTool = {
  name: 'getPageTitle',
  description: 'Gets the title of the current browser page.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId } = args as { sessionId: string };
    logger.debug('Executing browser_getPageTitle', { sessionId });

    if (!executeScript) {
      throw new Error('executeScript function is required for getPageTitle');
    }

    return executeScript(sessionId, 'document.title');
  },
};
