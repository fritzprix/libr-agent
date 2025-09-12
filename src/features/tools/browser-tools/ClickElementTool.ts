import { clickElement } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import {
  checkElementState,
  pollWithTimeout,
  formatBrowserResultAsMCP,
  BROWSER_TOOL_SCHEMAS,
} from './helpers';
import { StrictBrowserMCPTool } from './types';
import type { ElementState } from './helpers';

const logger = getLogger('ClickElementTool');

function createDetailedFailureText(
  elementState: ElementState,
  selector: string,
  sessionId: string,
): string {
  // Element not found case
  if (!elementState.exists) {
    return `Click failed: Element not found
Selector: ${selector}
Page state: No matching elements exist in current DOM
Session: ${sessionId}`;
  }

  // Element found but has specific issues
  const elementDesc = `${elementState.tagName || 'unknown'}${
    elementState.attributes?.id ? '#' + elementState.attributes.id : ''
  }${
    elementState.attributes?.className
      ? '.' + elementState.attributes.className.split(' ')[0]
      : ''
  }`;

  // Disabled element - fix: use 'disabled' property instead of 'enabled'
  if (elementState.disabled) {
    return `Click failed: Element is disabled
Selector: ${selector}
Element: ${elementDesc}
State: disabled="${elementState.attributes?.disabled || false}"
Reason: Element exists but cannot accept user interaction`;
  }

  // Visibility issues
  if (!elementState.visible) {
    const rect = elementState.rect;
    if (rect && rect.width === 0 && rect.height === 0) {
      return `Click failed: Element has zero dimensions
Selector: ${selector}
Element: ${elementDesc}
Computed size: 0x0px
Cause: CSS styling makes element effectively invisible`;
    }

    if (rect && (rect.x < 0 || rect.y < 0)) {
      return `Click failed: Element positioned outside viewport
Selector: ${selector}
Element: ${elementDesc}
Position: (${rect.x}, ${rect.y})
Viewport: Element coordinates are negative (off-screen)`;
    }

    if (rect && rect.width < 1 && rect.height < 1) {
      return `Click failed: Element too small to interact with
Selector: ${selector}
Element: ${elementDesc}
Dimensions: ${rect.width.toFixed(2)}x${rect.height.toFixed(2)}px
State: Element exists but rendered area is below interaction threshold`;
    }

    return `Click failed: Element not visible
Selector: ${selector}
Element: ${elementDesc}
Dimensions: ${rect?.width || 0}x${rect?.height || 0}px
Position: (${rect?.x || 0}, ${rect?.y || 0})
Visibility: CSS properties or positioning prevent user interaction`;
  }

  // Clickability issues - element visible but not clickable
  if (elementState.exists && elementState.visible && !elementState.clickable) {
    const rect = elementState.rect;

    return `Click failed: Element visible but not clickable
Selector: ${selector}
Element: ${elementDesc}
Position: (${rect?.x || 0}, ${rect?.y || 0})
Size: ${rect?.width || 0}x${rect?.height || 0}px
State: Element passes visibility checks but fails clickability validation
Possible causes: pointer-events:none, element covered, or event handlers blocked`;
  }

  // Generic failure with all available information
  return `Click failed: Unknown interaction issue
Selector: ${selector}
Element: ${elementDesc}
Status: exists=${elementState.exists}, visible=${elementState.visible}, clickable=${elementState.clickable}, disabled=${elementState.disabled || false}
Dimensions: ${elementState.rect?.width || 0}x${elementState.rect?.height || 0}px
Position: (${elementState.rect?.x || 0}, ${elementState.rect?.y || 0})
Session: ${sessionId}`;
}

function createSuccessText(
  selector: string,
  sessionId: string,
  result: string,
): string {
  try {
    const parsed = JSON.parse(result);
    if (parsed && typeof parsed === 'object' && parsed.ok) {
      let successMsg = `Click successful
Target: ${selector}
Result: Element interaction completed`;

      if (parsed.note) {
        successMsg += `\nResponse: ${parsed.note}`;
      }

      if (parsed.diagnostics) {
        successMsg += `\nDiagnostics: ${JSON.stringify(parsed.diagnostics)}`;
      }

      return successMsg;
    }
  } catch {
    // Fall through to default
  }

  return `Click successful
Target: ${selector}
Session: ${sessionId}`;
}

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

    if (typeof sessionId !== 'string' || !sessionId.trim()) {
      return formatBrowserResultAsMCP(
        'Click failed: Invalid sessionId parameter - must be non-empty string',
      );
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      return formatBrowserResultAsMCP(`Click failed: Invalid selector parameter - must be non-empty string
Session: ${sessionId}`);
    }

    logger.debug('Executing clickElement', { sessionId, selector });

    if (!executeScript) {
      return formatBrowserResultAsMCP(`Click failed: Missing executeScript dependency
Selector: ${selector}
Session: ${sessionId}
System error: Browser script execution function not available`);
    }

    try {
      // Validate element state with detailed analysis
      const elementState = await checkElementState(
        executeScript,
        sessionId,
        selector,
        'click',
      );

      if (!elementState) {
        return formatBrowserResultAsMCP(`Click failed: Element state validation failed
Selector: ${selector}
Session: ${sessionId}
System error: Unable to determine element state`);
      }

      // Check all failure conditions with detailed feedback
      if (
        !elementState.exists ||
        !elementState.visible ||
        !elementState.clickable ||
        elementState.disabled
      ) {
        const detailedFailure = createDetailedFailureText(
          elementState,
          selector,
          sessionId,
        );
        logger.debug('Element validation failed', { elementState, selector });
        return formatBrowserResultAsMCP(detailedFailure);
      }

      // Element passes validation - attempt click
      const requestId = await clickElement(sessionId, selector);
      logger.debug('Click request submitted', { requestId, selector });

      // Poll for result
      const result = await pollWithTimeout(requestId);

      if (result) {
        const successText = createSuccessText(selector, sessionId, result);
        logger.debug('Click completed successfully', { selector, result });
        return formatBrowserResultAsMCP(successText);
      }

      // Timeout case
      return formatBrowserResultAsMCP(`Click failed: Operation timeout
Selector: ${selector}
Session: ${sessionId}
Timeout: No response received from browser within expected timeframe
Element status: Validated successfully but click operation did not complete`);
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Click execution error', {
        error: errorMessage,
        sessionId,
        selector,
      });

      return formatBrowserResultAsMCP(`Click failed: Execution error
Selector: ${selector}
Session: ${sessionId}
Error: ${errorMessage}
Context: System error during click operation`);
    }
  },
};
