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

    const script = `
      const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
      el ? el.getAttribute('${attribute}') : null
    `;
    return executeScript(sessionId, script);
  },
};
