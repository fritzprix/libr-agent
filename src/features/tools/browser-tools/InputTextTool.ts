import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { StrictBrowserMCPTool } from './types';
import {
  validateSessionId,
  handleBrowserError,
  createBrowserErrorResponse,
} from './error-utils';

const logger = getLogger('InputTextTool');

export const inputTextTool: StrictBrowserMCPTool = {
  name: 'inputText',
  description: 'Inputs text into a form field using CSS selector.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: BROWSER_TOOL_SCHEMAS.selector,
      text: BROWSER_TOOL_SCHEMAS.text,
    },
    required: ['sessionId', 'selector', 'text'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const sessionId = args.sessionId;
    const selector = args.selector;
    const text = args.text;

    // Input validation
    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      return createBrowserErrorResponse(
        `✗ Input failed: Invalid selector parameter - must be non-empty string (session: ${sessionId})`,
      );
    }

    if (typeof text !== 'string') {
      return createBrowserErrorResponse(
        `✗ Input failed: Invalid text parameter - must be string (session: ${sessionId}, selector: ${selector})`,
      );
    }

    if (!executeScript) {
      return createBrowserErrorResponse(
        '✗ Input failed: executeScript function is required',
      );
    }

    logger.debug('Executing inputText', { sessionId, selector, text });

    try {
      // Simple script like inject_javascript - just set the value
      const script = `(function() { const el = document.querySelector(${JSON.stringify(selector)}); if (el) { el.value = ${JSON.stringify(text)}; el.dispatchEvent(new Event('input', {bubbles: true})); el.dispatchEvent(new Event('change', {bubbles: true})); } return el ? 'Input successful: ' + el.value : 'Element not found'; })()`;

      // Execute script using the provided executeScript function
      const result = await executeScript(sessionId as string, script);

      logger.debug('Input completed', { selector, result });
      return createMCPTextResponse(
        `✓ Input ${result.startsWith('Input successful') ? 'successful' : 'failed'} (selector: ${selector})\nResult: ${result}`,
      );
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'inputText',
        sessionId: sessionId as string,
        selector: selector as string,
      });
    }
  },
};
