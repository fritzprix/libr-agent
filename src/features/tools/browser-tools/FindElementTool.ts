import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';

const logger = getLogger('FindElementTool');

export const findElementTool: BrowserLocalMCPTool = {
  name: 'findElement',
  description:
    'Find element and check its state (existence, visibility, interactability)',
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
    logger.debug('Executing browser_findElement', {
      sessionId,
      selector,
    });

    if (!executeScript) {
      throw new Error('executeScript function is required for findElement');
    }

    const script = `
(function() {
  const selector = '${selector.replace(/'/g, "\\'")}';
  try {
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({ exists: false, selector });

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    const clickable = visible && style.pointerEvents !== 'none' && !el.disabled;

    return JSON.stringify({
      exists: true,
      visible,
      clickable,
      tagName: el.tagName.toLowerCase(),
      rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      attributes: {
        id: el.id || null,
        className: el.className || null,
        disabled: el.disabled || false
      },
      selector
    });
  } catch (error) {
    return JSON.stringify({ exists: false, error: error.message, selector });
  }
})()`;

    return await executeScript(sessionId, script);
  },
};
