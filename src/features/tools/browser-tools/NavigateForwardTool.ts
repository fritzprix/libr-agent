import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  validateSessionId,
  handleBrowserError,
  createBrowserErrorResponse,
} from './error-utils';

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

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    logger.debug('Executing browser_navigateForward', { sessionId });

    if (!executeScript) {
      return createBrowserErrorResponse(
        '✗ Navigate Forward failed: executeScript function is required',
      );
    }

    try {
      const result = await executeScript(
        sessionId,
        'setTimeout(() => history.forward(), 10); "Navigated forward"',
      );
      return createMCPTextResponse(result);
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'navigateForward',
        sessionId,
      });
    }
  },
};
