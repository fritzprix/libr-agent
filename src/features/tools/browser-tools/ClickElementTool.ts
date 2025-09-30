import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { StrictBrowserMCPTool } from './types';

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
    if (typeof sessionId !== 'string' || !sessionId.trim()) {
      return createMCPTextResponse(
        '✗ Click failed: Invalid sessionId parameter - must be non-empty string',
      );
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      return createMCPTextResponse(
        `✗ Click failed: Invalid selector parameter - must be non-empty string (session: ${sessionId})`,
      );
    }

    if (!executeScript) {
      return createMCPTextResponse(
        '✗ Click failed: executeScript function is required',
      );
    }

    logger.debug('Executing clickElement', { sessionId, selector });

    try {
      // Simple script like inject_javascript - just click the element
      const script = `(function() { const el = document.querySelector(${JSON.stringify(selector)}); if (el) { el.scrollIntoView({block: 'center'}); el.focus(); el.click(); } return el ? 'Clicked element' : 'Element not found'; })()`;

      // Execute script using the provided executeScript function
      const result = await executeScript(sessionId, script);

      logger.debug('Click completed', { selector, result });
      return createMCPTextResponse(
        `✓ Click ${result === 'Clicked element' ? 'successful' : 'failed'} (selector: ${selector})\nResult: ${result}`,
      );
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Click execution error', {
        error: errorMessage,
        sessionId,
        selector,
      });

      return createMCPTextResponse(
        `✗ Click failed: ${errorMessage} (selector: ${selector}, session: ${sessionId})`,
      );
    }
  },
};
