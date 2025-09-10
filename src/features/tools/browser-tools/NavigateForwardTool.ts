import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';

const logger = getLogger('NavigateForwardTool');

export const navigateForwardTool: StrictBrowserMCPTool = {
  name: 'navigateForward',
  description: 'Navigate forward in browser history',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId } = args as { sessionId: string };
    logger.debug('Executing browser_navigateForward', { sessionId });

    if (!executeScript) {
      throw new Error('executeScript function is required for navigateForward');
    }

    const result = await executeScript(
      sessionId,
      'history.forward(); "Navigated forward"',
    );
    return createMCPTextResponse(result);
  },
};
