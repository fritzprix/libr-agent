import { closeBrowserSession } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictLocalMCPTool } from './types';

const logger = getLogger('CloseSessionTool');

export const closeSessionTool: StrictLocalMCPTool = {
  name: 'closeSession',
  description: 'Closes an existing browser session and its window.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId } = args as { sessionId: string };
    logger.debug('Executing browser_closeSession', { sessionId });

    await closeBrowserSession(sessionId);
    return createMCPTextResponse(`Browser session closed: ${sessionId}`);
  },
};
