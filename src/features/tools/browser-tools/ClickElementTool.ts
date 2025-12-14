import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import {
  createMCPErrorResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';
import { FailureTracker } from './failure-tracker';

const logger = getLogger('ClickElementTool');

export const clickElementTool: StrictBrowserMCPTool = {
  name: 'clickElement',
  description:
    'Clicks on a DOM element using CSS selector with detailed failure analysis.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: BROWSER_TOOL_SCHEMAS.selector,
    },
    required: ['sessionId', 'selector'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const sessionId = args.sessionId;
    const selector = args.selector;

    // Input validation
    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      return createMCPErrorResponse(
        `✗ Click failed: Invalid selector parameter - must be non-empty string (session: ${sessionId})`,
      );
    }

    if (!executeScript) {
      return createMCPErrorResponse(
        '✗ Click failed: executeScript function is required',
      );
    }

    logger.debug('Executing clickElement', { sessionId, selector });

    try {
      // Simple script like inject_javascript - just click the element
      const script = `(function() { const el = document.querySelector(${JSON.stringify(selector)}); if (el) { el.scrollIntoView({block: 'center'}); el.focus(); el.click(); } return el ? 'Clicked element' : 'Element not found'; })()`;

      // Execute script using the provided executeScript function
      const result = await executeScript(sessionId as string, script);

      logger.debug('Click completed', { selector, result });

      FailureTracker.resetFailure(sessionId as string);

      return createMCPTextResponse(
        `✓ Click ${result === 'Clicked element' ? 'successful' : 'failed'} (selector: ${selector})\nResult: ${result}`,
      );
    } catch (error) {
      const failureCount = FailureTracker.recordFailure(sessionId as string);
      return handleBrowserError(error, {
        toolName: 'clickElement',
        sessionId: sessionId as string,
        selector: selector as string,
        failureCount,
      });
    }
  },
};
