import { formatBrowserResultAsMCP } from './helpers';
import type { StrictBrowserMCPTool } from './types';
import { validateSessionId, handleBrowserError } from './error-utils';

/**
 * Tool: inject_javascript
 * Injects and executes arbitrary JavaScript code in the specified browser session.
 * Returns the result or error from the script execution.
 */
export const injectJavascriptTool: StrictBrowserMCPTool = {
  name: 'inject_javascript',
  description:
    'Inject and execute arbitrary JavaScript code in a browser session. Returns the result or error.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: {
        type: 'string',
        description: 'ID of the browser session',
      },
      script: {
        type: 'string',
        description: 'JavaScript code to execute',
      },
    },
    required: ['sessionId', 'script'],
  },
  async execute(args: Record<string, unknown>, executeScript) {
    const sessionId = String(args.sessionId || '');
    const script = String(args.script || '');

    const { isValid, errorResponse } = validateSessionId(sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    if (!script) {
      return formatBrowserResultAsMCP(
        'inject_javascript failed: script is required',
      );
    }

    if (!executeScript) {
      return formatBrowserResultAsMCP(
        'inject_javascript failed: executeScript function is required',
      );
    }

    try {
      const result = await executeScript(sessionId, script);
      return formatBrowserResultAsMCP(
        `✓ JAVASCRIPT injected and executed successfully in session ${sessionId}\nResult: ${result}`,
      );
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'inject_javascript',
        sessionId,
      });
    }
  },
};
