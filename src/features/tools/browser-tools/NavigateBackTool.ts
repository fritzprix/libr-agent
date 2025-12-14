import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS, pollCondition } from './helpers';
import { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';

const logger = getLogger('NavigateBackTool');

export const navigateBackTool: StrictBrowserMCPTool = {
  name: 'navigateBack',
  description:
    'Navigate back in browser history. Waits for the page to load before returning.',
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
      // Trigger navigation
      await executeScript(sessionId, 'history.back()');

      // Wait a bit for navigation to start
      await new Promise((resolve) => setTimeout(resolve, 500));

      // Poll for page load completion
      await pollCondition(
        async () => {
          try {
            const readyState = await executeScript(
              sessionId,
              'document.readyState',
            );
            return readyState === 'complete';
          } catch {
            // If execution fails, it might be during navigation, so we continue polling
            return false;
          }
        },
        10000, // 10s timeout
        500, // 500ms interval
      );

      return createMCPTextResponse('✓ Navigated back and page loaded');
    } catch (error) {
      return handleBrowserError(error, { toolName: 'navigateBack', sessionId });
    }
  },
};
