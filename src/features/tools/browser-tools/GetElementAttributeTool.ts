import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('GetElementAttributeTool');

export const getElementAttributeTool: BrowserLocalMCPTool = {
  name: 'getElementAttribute',
  description: 'Get specific attribute value of an element',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: BROWSER_TOOL_SCHEMAS.selector,
      attribute: {
        type: 'string',
        description: 'The attribute name to retrieve',
      },
    },
    required: ['sessionId', 'selector', 'attribute'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const { sessionId, selector, attribute } = args as {
      sessionId: string;
      selector: string;
      attribute: string;
    };
    logger.debug('Executing browser_getElementAttribute', {
      sessionId,
      selector,
      attribute,
    });

    if (!executeScript) {
      throw new Error(
        'executeScript function is required for getElementAttribute',
      );
    }

    // Improved script with JSON response structure
    const script = `
(function() {
  try {
    const selector = ${JSON.stringify(selector)};
    const attribute = ${JSON.stringify(attribute)};
    const el = document.querySelector(selector);
    
    if (!el) {
      return JSON.stringify({
        success: false,
        value: null,
        error: 'Element not found',
        selector: selector
      });
    }
    
    const attributeValue = el.getAttribute(attribute);
    return JSON.stringify({
      success: true,
      value: attributeValue,
      selector: selector,
      attribute: attribute
    });
  } catch (error) {
    return JSON.stringify({
      success: false,
      value: null,
      error: error.message,
      selector: ${JSON.stringify(selector)},
      attribute: ${JSON.stringify(attribute)}
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
        logger.warn('Element attribute retrieval failed', {
          error: parsedResult.error,
          selector,
          attribute,
        });
        return null;
      }
    } catch (error) {
      logger.error('Error executing script', {
        error,
        sessionId,
        selector,
        attribute,
      });
      return null;
    }
  },
};
