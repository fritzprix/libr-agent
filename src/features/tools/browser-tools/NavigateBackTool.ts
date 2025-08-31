import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('NavigateBackTool');

export const navigateBackTool: BrowserLocalMCPTool = {
  name: 'navigateBack',
  description: 'Navigate back in browser history',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId } = args as { sessionId: string };
    logger.debug('Executing browser_navigateBack', { sessionId });

    if (!executeScript) {
      throw new Error('executeScript function is required for navigateBack');
    }

    return executeScript(sessionId, 'history.back(); "Navigated back"');
  },
};
