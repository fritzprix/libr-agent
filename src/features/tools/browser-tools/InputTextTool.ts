import { inputText } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import {
  checkElementState,
  pollWithTimeout,
  formatBrowserResult,
  BROWSER_TOOL_SCHEMAS,
} from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('InputTextTool');

export const inputTextTool: BrowserLocalMCPTool = {
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
    const { sessionId, selector, text } = args as {
      sessionId: string;
      selector: string;
      text: string;
    };
    logger.debug('Executing browser_inputText', { sessionId, selector });

    if (!executeScript) {
      throw new Error('executeScript function is required for inputText');
    }

    try {
      // Check element state using common helper with input-specific validation
      const elementState = await checkElementState(
        executeScript,
        sessionId,
        selector,
        'input',
      );

      if (!elementState.exists) {
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'input',
            reason: 'element_not_found',
            selector,
          }),
        );
      }

      if (!elementState.inputable) {
        return formatBrowserResult(
          JSON.stringify({
            ok: false,
            action: 'input',
            reason: 'element_not_inputable',
            selector,
            diagnostics: elementState,
          }),
        );
      }

      // Element is valid, proceed with input
      const requestId = await inputText(sessionId, selector, text);
      logger.debug('Input text request ID received', { requestId });

      // Poll for result with timeout using common helper
      const result = await pollWithTimeout(requestId);

      if (result) {
        logger.debug('Poll result received', { result });
        return formatBrowserResult(result);
      }

      return 'Input operation timed out - no response received from browser';
    } catch (error) {
      logger.error('Error in input_text execution', {
        error,
        sessionId,
        selector,
      });
      return `Error executing input: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
};
