import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';

const logger = getLogger('ScrollPageTool');

export const scrollPageTool: StrictBrowserMCPTool = {
  name: 'scrollPage',
  description: 'Scrolls the page to specified coordinates.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      x: {
        type: 'number',
        description: 'X coordinate to scroll to.',
      },
      y: {
        type: 'number',
        description: 'Y coordinate to scroll to.',
      },
    },
    required: ['sessionId', 'x', 'y'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId, x, y } = args as {
      sessionId: string;
      x: number;
      y: number;
    };
    logger.debug('Executing browser_scrollPage', { sessionId, x, y });

    if (!executeScript) {
      throw new Error('executeScript function is required for scrollPage');
    }

    const script = `window.scrollTo(${x}, ${y}); 'Scrolled to (${x}, ${y})'`;
    const result = await executeScript(sessionId, script);
    return createMCPTextResponse(result);
  },
};
