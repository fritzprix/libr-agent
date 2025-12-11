import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';

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

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    logger.debug('Executing browser_getCurrentUrl', { sessionId });

    if (!executeScript) {
      return createMCPTextResponse(
        '✗ Get URL failed: executeScript function is required',
      );
    }

    try {
      const result = await executeScript(sessionId, 'window.location.href');
      return createMCPTextResponse(result);
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'getCurrentUrl',
        sessionId,
      });
    }
  },
};
