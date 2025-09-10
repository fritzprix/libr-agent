import { clickElement } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import {
  checkElementState,
  pollWithTimeout,
  formatBrowserResult,
  BROWSER_TOOL_SCHEMAS,
} from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('ClickElementTool');

export const clickElementTool: BrowserLocalMCPTool = {
  name: 'clickElement',
  description: 'Clicks on a DOM element using CSS selector.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: BROWSER_TOOL_SCHEMAS.selector,
    },
    required: ['sessionId', 'selector'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    // Input validation without type casting
    const sessionId = args.sessionId;
    const selector = args.selector;

    if (typeof sessionId !== 'string' || !sessionId.trim()) {
      const error = 'Invalid sessionId: must be a non-empty string';
      logger.error(error, { args });
      return formatBrowserResult(
        JSON.stringify({
          ok: false,
          action: 'click',
          reason: 'invalid_input',
          error,
        }),
      );
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      const error = 'Invalid selector: must be a non-empty string';
      logger.error(error, { args });
      return formatBrowserResult(
        JSON.stringify({
          ok: false,
          action: 'click',
          reason: 'invalid_input',
          error,
          sessionId,
        }),
      );
    }

    logger.debug('Executing browser_clickElement', {
      sessionId,
      selector,
    });

    if (!executeScript) {
      const error = 'executeScript function is required for clickElement';
      logger.error(error);
      return formatBrowserResult(
        JSON.stringify({
          ok: false,
          action: 'click',
          reason: 'missing_dependency',
          error,
          sessionId,
          selector,
        }),
      );
    }

    try {
      // Check element state using common helper
      const elementState = await checkElementState(
        executeScript,
        sessionId,
        selector,
        'click',
      );

      // Additional safety check for null elementState
      if (!elementState) {
        const error = 'Element state check returned null';
        logger.error(error, { sessionId, selector });
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'click',
            reason: 'element_check_failed',
            error,
            sessionId,
            selector,
          }),
        );
      }

      if (!elementState.exists) {
        logger.debug('Element not found', { elementState });
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'click',
            reason: 'element_not_found',
            selector,
            sessionId,
            diagnostics: elementState,
          }),
        );
      }

      if (!elementState.clickable) {
        logger.debug('Element not clickable', { elementState });
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'click',
            reason: 'element_not_clickable',
            selector,
            sessionId,
            diagnostics: elementState,
          }),
        );
      }

      // Element is valid, proceed with click
      const requestId = await clickElement(sessionId, selector);
      logger.debug('Click element request ID received', { requestId });

      // Poll for result with timeout using common helper
      const result = await pollWithTimeout(requestId);

      if (result) {
        logger.debug('Poll result received', { result });
        return formatBrowserResult(result);
      }

      // Timeout case - return consistent error format
      const timeoutError =
        'Click operation timed out - no response received from browser';
      logger.warn(timeoutError, { sessionId, selector, requestId });
      return formatBrowserResult(
        JSON.stringify({
          ok: false,
          action: 'click',
          reason: 'timeout',
          error: timeoutError,
          sessionId,
          selector,
        }),
      );
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Error in click_element execution', {
        error: errorMessage,
        sessionId,
        selector,
      });

      return formatBrowserResult(
        JSON.stringify({
          ok: false,
          action: 'click',
          reason: 'execution_error',
          error: errorMessage,
          sessionId,
          selector,
        }),
      );
    }
  },
};
