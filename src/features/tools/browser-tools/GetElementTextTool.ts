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

    const script = `
      const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
      el ? el.textContent.trim() : null
    `;
    return executeScript(sessionId, script);
  },
};
