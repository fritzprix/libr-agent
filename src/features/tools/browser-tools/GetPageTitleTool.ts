import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  validateSessionId,
  handleBrowserError,
  createBrowserErrorResponse,
} from './error-utils';

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

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    logger.debug('Executing browser_getPageTitle', { sessionId });

    if (!executeScript) {
      return createBrowserErrorResponse(
        '✗ Get Title failed: executeScript function is required',
      );
    }

    try {
      const result = await executeScript(sessionId, 'document.title');
      return createMCPTextResponse(result);
    } catch (error) {
      return handleBrowserError(error, { toolName: 'getPageTitle', sessionId });
    }
  },
};
