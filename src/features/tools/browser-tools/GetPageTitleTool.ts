import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';

const logger = getLogger('GetPageTitleTool');

export const getPageTitleTool: StrictBrowserMCPTool = {
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

    const result = await executeScript(sessionId, 'document.title');
    return createMCPTextResponse(result);
  },
};
