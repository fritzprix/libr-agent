import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS, pollCondition } from './helpers';
import {
  createMCPErrorResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';
import { FailureTracker } from './failure-tracker';

const logger = getLogger('ClickElementTool');

export const clickElementTool: StrictBrowserMCPTool = {
  name: 'clickElement',
  description:
    'Clicks on a DOM element using CSS selector. Automatically waits for page load if the click triggers navigation (links, form submissions). Use waitForNavigation parameter to override automatic detection.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: BROWSER_TOOL_SCHEMAS.selector,
      waitForNavigation: {
        type: 'string',
        description:
          'Navigation wait strategy: "auto" (default, smart detection for links/forms), "always" (always wait), "never" (never wait)',
        enum: ['auto', 'always', 'never'],
      },
      navigationTimeout: {
        type: 'number',
        description:
          'Maximum wait time for navigation completion in milliseconds (default: 10000)',
      },
    },
    required: ['sessionId', 'selector'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const sessionId = args.sessionId as string;
    const selector = args.selector as string;
    const waitForNavigation = (args.waitForNavigation as string) || 'auto';
    const navigationTimeout = (args.navigationTimeout as number) || 10000;

    // Input validation
    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    if (typeof selector !== 'string' || !selector.trim()) {
      return createMCPErrorResponse(
        `✗ Click failed: Invalid selector parameter - must be non-empty string (session: ${sessionId})`,
      );
    }

    if (!executeScript) {
      return createMCPErrorResponse(
        '✗ Click failed: executeScript function is required',
      );
    }

    logger.debug('Executing clickElement', {
      sessionId,
      selector,
      waitForNavigation,
      navigationTimeout,
    });

    try {
      // Enhanced click script that returns element info for navigation detection
      const clickScript = `(function() {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (el) {
          el.scrollIntoView({block: 'center'});
          el.focus();
          
          // Collect info before click
          const tagName = el.tagName.toLowerCase();
          const type = el.getAttribute('type')?.toLowerCase();
          const href = el.getAttribute('href');
          const isSubmit = (tagName === 'button' || tagName === 'input') && type === 'submit';
          const isLink = tagName === 'a' && !!href;
          
          el.click();
          
          return JSON.stringify({
            status: 'Clicked element',
            isLink,
            isSubmit,
            tagName
          });
        }
        return JSON.stringify({ status: 'Element not found' });
      })()`;

      const resultStr = await executeScript(sessionId, clickScript);
      let result;
      try {
        result = JSON.parse(resultStr);
      } catch {
        // Fallback if not JSON (should not happen with above script)
        result = { status: resultStr };
      }

      if (result.status !== 'Clicked element') {
        return createMCPTextResponse(
          `✗ Click failed: ${result.status} (selector: ${selector})\n\nTip: If the element is difficult to click, try using 'extractWebContent' to analyze the page structure again, or use 'navigateToUrl' to go directly to the target page.`,
        );
      }

      logger.debug('Click executed', { selector, result });

      // Determine if we should wait for navigation
      let shouldWait = false;
      let detectionReason = 'User override';

      if (waitForNavigation === 'always') {
        shouldWait = true;
        detectionReason = 'User requested always wait';
      } else if (waitForNavigation === 'never') {
        shouldWait = false;
        detectionReason = 'User requested never wait';
      } else {
        // Auto detection
        if (result.isLink || result.isSubmit) {
          shouldWait = true;
          detectionReason = result.isLink
            ? 'Link detected'
            : 'Submit button detected';
        } else {
          detectionReason = 'No navigation element detected';
        }
      }

      if (shouldWait) {
        logger.debug('Waiting for page load', {
          selector,
          timeout: navigationTimeout,
        });

        // Wait a bit for navigation to start (similar to NavigateForwardTool)
        await new Promise((resolve) => setTimeout(resolve, 500));

        // Poll for page load completion
        const loaded = await pollCondition(
          async () => {
            try {
              const readyState = await executeScript(
                sessionId,
                'document.readyState',
              );
              return readyState === 'complete';
            } catch {
              // If execution fails, it might be during navigation, so we continue polling
              return false;
            }
          },
          navigationTimeout,
          500,
        );

        if (!loaded) {
          logger.warn('Page load timeout', {
            selector,
            timeout: navigationTimeout,
          });
          return createMCPTextResponse(
            `⚠ Click successful but page load timeout after ${navigationTimeout}ms (selector: ${selector})\nReason: ${detectionReason}`,
          );
        }

        FailureTracker.resetFailure(sessionId);

        return createMCPTextResponse(
          `✓ Click successful and page loaded (selector: ${selector})\nNavigation detected: ${detectionReason}`,
        );
      } else {
        FailureTracker.resetFailure(sessionId);

        return createMCPTextResponse(
          `✓ Click successful (selector: ${selector})\nNo navigation expected: ${detectionReason}`,
        );
      }
    } catch (error) {
      const failureCount = FailureTracker.recordFailure(sessionId);
      const errorResponse = handleBrowserError(error, {
        toolName: 'clickElement',
        sessionId,
        selector,
        failureCount,
      });

      // Add guidance to the error message
      if (
        errorResponse.result &&
        errorResponse.result.content &&
        errorResponse.result.content[0] &&
        errorResponse.result.content[0].type === 'text'
      ) {
        errorResponse.result.content[0].text +=
          "\n\nTip: If the element is difficult to click, try using 'extractWebContent' to analyze the page structure again, or use 'navigateToUrl' to go directly to the target page.";
      }

      return errorResponse;
    }
  },
};
