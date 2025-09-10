import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';

const logger = getLogger('GetCurrentUrlTool');

export const getCurrentUrlTool: StrictBrowserMCPTool = {
  name: 'getCurrentUrl',
  description: 'Gets the current URL of the browser page.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId } = args as { sessionId: string };
    logger.debug('Executing browser_getCurrentUrl', { sessionId });

    if (!executeScript) {
      throw new Error('executeScript function is required for getCurrentUrl');
    }

    const result = await executeScript(sessionId, 'window.location.href');
    return createMCPTextResponse(result);
  },
};
