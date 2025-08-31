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
    const { sessionId, selector } = args as {
      sessionId: string;
      selector: string;
    };
    logger.debug('Executing browser_clickElement', {
      sessionId,
      selector,
    });

    if (!executeScript) {
      throw new Error('executeScript function is required for clickElement');
    }

    try {
      // Check element state using common helper
      const elementState = await checkElementState(
        executeScript,
        sessionId,
        selector,
        'click',
      );

      if (!elementState.exists) {
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'click',
            reason: 'element_not_found',
            selector,
          }),
        );
      }

      if (!elementState.clickable) {
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'click',
            reason: 'element_not_clickable',
            selector,
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

      return 'Click operation timed out - no response received from browser';
    } catch (error) {
      logger.error('Error in click_element execution', {
        error,
        sessionId,
        selector,
      });
      return `Error executing click: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
};
