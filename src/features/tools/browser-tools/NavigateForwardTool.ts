import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('NavigateForwardTool');

export const navigateForwardTool: BrowserLocalMCPTool = {
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

    return executeScript(sessionId, 'history.forward(); "Navigated forward"');
  },
};
