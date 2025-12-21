import { getLogger } from '@/lib/logger';
import { createMCPErrorResponse } from '@/lib/mcp-response-utils';
import { MCPResponse } from '@/lib/mcp-types';
import {
  BrowserError,
  BrowserErrorCode,
  parseBrowserError,
  getBrowserErrorMessage,
  isBrowserError,
} from './browser-error';

const logger = getLogger('BrowserErrorUtils');

// Legacy alias for backward compatibility
export const createBrowserErrorResponse = createMCPErrorResponse;

/**
 * Validates the sessionId parameter.
 * @param sessionId The session ID to validate.
 * @returns An object indicating validity and an optional error response.
 */
export function validateSessionId(sessionId: unknown): {
  isValid: boolean;
  errorResponse?: MCPResponse<unknown>;
} {
  if (typeof sessionId !== 'string' || !sessionId.trim()) {
    return {
      isValid: false,
      errorResponse: createMCPErrorResponse(
        '✗ Invalid sessionId parameter - must be a non-empty string. Please check the session ID from listSessions.',
      ),
    };
  }
  return { isValid: true };
}

/**
 * Handles errors from browser tools and provides actionable guidance.
 * Supports both structured BrowserError types and legacy string errors.
 * @param error The error object or message.
 * @param context Context information for the error (tool name, session ID, selector, etc.).
 * @returns An MCPResponse with the error message and guidance.
 */
export function handleBrowserError(
  error: unknown,
  context: {
    toolName: string;
    sessionId?: string;
    selector?: string;
    failureCount?: number;
  },
): MCPResponse<unknown> {
  const { toolName, selector, failureCount } = context;

  logger.error(`Error in ${toolName}`, { error, context });

  // Try to parse as structured error
  const parsedError = parseBrowserError(error);

  let errorMessage: string;
  let guidance = '';

  // Handle structured browser errors
  if (isBrowserError(parsedError)) {
    errorMessage = getBrowserErrorMessage(parsedError);
    guidance = getGuidanceForError(parsedError, selector);
  }
  // Handle legacy string errors (backward compatibility)
  else {
    errorMessage =
      typeof parsedError === 'string' ? parsedError : String(error);
    guidance = getLegacyGuidance(errorMessage, selector);
  }

  // Add suggestion for repeated failures
  if (failureCount && failureCount >= 3) {
    guidance +=
      '\n\n(suggestion: You have failed to interact with the page 3 consecutive times. You might want to consider using `navigateToUrl` to reload the page or go to a different URL.)';
  }

  const finalMessage = `✗ ${toolName} failed: ${errorMessage}\n\nGuidance: ${guidance || 'Please check the tool parameters and try again.'}`;

  return createMCPErrorResponse(finalMessage);
}

/**
 * Get actionable guidance based on structured error type
 */
function getGuidanceForError(error: BrowserError, selector?: string): string {
  switch (error.code) {
    case BrowserErrorCode.SESSION_NOT_FOUND:
      return 'The browser session does not exist. Please use `listSessions` to verify active sessions or `createSession` to start a new one.';

    case BrowserErrorCode.SESSION_CLOSED:
      return 'The browser session was already closed. Please create a new session with `createSession`.';

    case BrowserErrorCode.WINDOW_NOT_FOUND:
      return 'The browser window was closed or not found. Please create a new session with `createSession`.';

    case BrowserErrorCode.ELEMENT_NOT_FOUND:
      return `The element with selector "${error.context.selector || selector || 'unknown'}" could not be found. Please use \`listInteractable\` to see available elements and their selectors on the current page.`;

    case BrowserErrorCode.ELEMENT_NOT_INTERACTABLE:
      return `The element with selector "${error.context.selector}" exists but cannot be interacted with (${error.context.reason}). Please check if the element is visible, enabled, and not obscured by other elements.`;

    case BrowserErrorCode.NAVIGATION_FAILED:
      return `Navigation to "${error.context.url}" failed (${error.context.reason}). Please check the URL format and your network connection. You can also try checking if the page loaded using \`getPageTitle\`.`;

    case BrowserErrorCode.SCRIPT_EXECUTION_FAILED:
      return `JavaScript execution failed (${error.context.reason}). The page might not be fully loaded yet, or the script might have syntax errors.`;

    case BrowserErrorCode.TIMEOUT:
      return `The operation "${error.context.operation}" took too long to complete (${error.context.duration_ms}ms). Please close the current session using \`closeSession\`, create a new session using \`createSession\`, and try visiting a different page.`;

    case BrowserErrorCode.LOCK_FAILED:
      return 'Internal error: Failed to acquire resource lock. This is likely a temporary issue, please try again.';

    case BrowserErrorCode.INVALID_PARAMETER:
      return `Invalid parameter "${error.context.parameter}": ${error.context.reason}. Please check the tool documentation for correct parameter format.`;

    case BrowserErrorCode.UNKNOWN:
      return 'An unexpected error occurred. Please check the error message for details.';

    default:
      return 'Please check the tool parameters and try again.';
  }
}

/**
 * Legacy guidance for string-based errors (backward compatibility)
 */
function getLegacyGuidance(errorMessage: string, selector?: string): string {
  // Session errors - Use exact matching for reliability
  if (errorMessage === 'Session not found') {
    return 'The browser session does not exist. Please use `listSessions` to verify active sessions or `createSession` to start a new one.';
  }
  // Browser window errors
  else if (errorMessage === 'Browser window not found') {
    return 'The browser window was closed or not found. Please create a new session with `createSession`.';
  }
  // Element not found (from structured JSON response)
  else if (
    errorMessage.includes('"reason":"not_found"') ||
    errorMessage.includes('not_found')
  ) {
    return `The element with selector "${selector || 'unknown'}" could not be found. Please use \`listInteractable\` to see available elements and their selectors on the current page.`;
  }
  // Content store errors
  else if (errorMessage.startsWith('No content found')) {
    return 'No content was extracted. The page might not be fully loaded yet. Try waiting a moment or using `readWebContent` to inspect the raw HTML.';
  }

  return 'Please check the tool parameters and try again.';
}
