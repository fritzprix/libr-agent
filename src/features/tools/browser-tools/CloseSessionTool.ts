import { closeBrowserSession } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictLocalMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';
import { FailureTracker } from './failure-tracker';

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

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    logger.debug('Executing browser_closeSession', { sessionId });

    try {
      await closeBrowserSession(sessionId);

      // Cleanup browser-session specific state (Failure counts)
      // ContentStore is intentionally NOT cleaned up here, as it belongs to the conversation scope
      FailureTracker.deleteSession(sessionId);

      return createMCPTextResponse(`Browser session closed: ${sessionId}`);
    } catch (error) {
      return handleBrowserError(error, { toolName: 'closeSession', sessionId });
    }
  },
};
