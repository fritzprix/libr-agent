import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('GetElementTextTool');

export const getElementTextTool: BrowserLocalMCPTool = {
  name: 'getElementText',
  description: 'Get text content of a specific element',
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
    logger.debug('Executing browser_getElementText', {
      sessionId,
      selector,
    });

    if (!executeScript) {
      throw new Error('executeScript function is required for getElementText');
    }

    // Improved script with JSON response structure
    const script = `
(function() {
  try {
    const selector = ${JSON.stringify(selector)};
    const el = document.querySelector(selector);
    
    if (!el) {
      return JSON.stringify({
        success: false,
        value: null,
        error: 'Element not found',
        selector: selector
      });
    }
    
    const textContent = el.textContent ? el.textContent.trim() : '';
    return JSON.stringify({
      success: true,
      value: textContent,
      selector: selector
    });
  } catch (error) {
    return JSON.stringify({
      success: false,
      value: null,
      error: error.message,
      selector: ${JSON.stringify(selector)}
    });
  }
})()
    `;

    try {
      const result = await executeScript(sessionId, script);
      logger.debug('Script execution result', { result });

      // Parse JSON response
      let parsedResult;
      try {
        parsedResult = JSON.parse(result);
      } catch (parseError) {
        logger.error('Failed to parse script result', { result, parseError });
        return null;
      }

      if (parsedResult.success) {
        return parsedResult.value;
      } else {
        logger.warn('Element text retrieval failed', {
          error: parsedResult.error,
          selector,
        });
        return null;
      }
    } catch (error) {
      logger.error('Error executing script', { error, sessionId, selector });
      return null;
    }
  },
};
