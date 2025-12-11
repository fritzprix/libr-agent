import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';

const logger = getLogger('NavigateBackTool');

export const navigateBackTool: StrictBrowserMCPTool = {
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

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    logger.debug('Executing browser_navigateBack', { sessionId });

    if (!executeScript) {
      return createMCPTextResponse(
        '✗ Navigate Back failed: executeScript function is required',
      );
    }

    try {
      const result = await executeScript(
        sessionId,
        'setTimeout(() => history.back(), 10); "Navigated back"',
      );
      return createMCPTextResponse(result);
    } catch (error) {
      return handleBrowserError(error, { toolName: 'navigateBack', sessionId });
    }
  },
};
