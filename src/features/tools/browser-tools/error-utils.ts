import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { getLogger } from '@/lib/logger';
import { MCPResponse } from '@/lib/mcp-types';

const logger = getLogger('BrowserErrorUtils');

/**
 * Creates an MCP response representing an error.
 * @param message The error message.
 * @returns An MCPResponse with isError set to true.
 */
export function createBrowserErrorResponse(
  message: string,
): MCPResponse<unknown> {
  const response = createMCPTextResponse(message);
  if (response.result) {
    response.result.isError = true;
  }
  return response;
}

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
      errorResponse: createBrowserErrorResponse(
        '✗ Invalid sessionId parameter - must be a non-empty string. Please check the session ID from listSessions.',
      ),
    };
  }
  return { isValid: true };
}

/**
 * Handles errors from browser tools and provides actionable guidance.
 * @param error The error object or message.
 * @param context Context information for the error (tool name, session ID, selector, etc.).
 * @returns An MCPResponse with the error message and guidance.
 */
export function handleBrowserError(
  error: unknown,
  context: { toolName: string; sessionId?: string; selector?: string },
): MCPResponse<unknown> {
  const errorMessage = error instanceof Error ? error.message : String(error);
  const { toolName, selector } = context;

  logger.error(`Error in ${toolName}`, { error, context });

  let guidance = '';

  // Session errors
  if (
    errorMessage.toLowerCase().includes('session not found') ||
    errorMessage.toLowerCase().includes('session closed') ||
    errorMessage.toLowerCase().includes('invalid session')
  ) {
    guidance =
      'The browser session might have been closed or does not exist. Please use `listSessions` to verify active sessions or `createSession` to start a new one.';
  }
  // Selector errors
  else if (
    errorMessage.toLowerCase().includes('element not found') ||
    (selector && errorMessage.toLowerCase().includes('selector'))
  ) {
    guidance = `The element with selector "${selector}" could not be found. Please use \`listInteractable\` to see available elements and their selectors on the current page.`;
  }
  // Navigation/Network errors
  else if (
    errorMessage.toLowerCase().includes('navigation') ||
    errorMessage.toLowerCase().includes('timeout') ||
    errorMessage.toLowerCase().includes('network')
  ) {
    guidance =
      'Navigation failed or timed out. Please check the URL format and your network connection. You can also try checking if the page loaded using `getPageTitle`.';
  }
  // Empty content
  else if (errorMessage.toLowerCase().includes('no content found')) {
    guidance =
      'No content was extracted. The page might not be fully loaded yet. Try waiting a moment or using `readWebContent` to inspect the raw HTML.';
  }

  const finalMessage = `✗ ${toolName} failed: ${errorMessage}\n\nGuidance: ${guidance || 'Please check the tool parameters and try again.'}`;

  return createBrowserErrorResponse(finalMessage);
}
