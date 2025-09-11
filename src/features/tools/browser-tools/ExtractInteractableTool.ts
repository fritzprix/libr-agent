import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { parseHtmlToInteractables } from '@/lib/html-parser';
import type { InteractableOptions } from '@/lib/html-parser';

const logger = getLogger('ExtractInteractableTool');

interface ValidatedArgs {
  sessionId: string;
  selector: string;
  includeHidden: boolean;
  maxElements: number;
}

// Simple HTML extraction function (following extractContent pattern)
async function extractHtmlFromPage(
  executeScript: (sessionId: string, script: string) => Promise<unknown>,
  sessionId: string,
  selector: string,
): Promise<string> {
  const rawHtml = await executeScript(
    sessionId,
    `document.querySelector(${JSON.stringify(selector)}).outerHTML`,
  );

  if (!rawHtml || typeof rawHtml !== 'string') {
    throw new Error(
      'Failed to extract HTML from the page - no content found or invalid content type',
    );
  }

  return rawHtml;
}

function validateExtractInteractableArgs(
  args: Record<string, unknown>,
): ValidatedArgs | null {
  logger.debug('Validating extractInteractable args:', args);

  if (typeof args.sessionId !== 'string') {
    logger.warn('Invalid sessionId type', {
      sessionId: args.sessionId,
      type: typeof args.sessionId,
    });
    return null;
  }

  const selector = args.selector ?? 'body';
  if (typeof selector !== 'string') {
    logger.warn('Invalid selector type', { selector, type: typeof selector });
    return null;
  }

  const includeHidden = args.includeHidden ?? false;
  if (typeof includeHidden !== 'boolean') {
    logger.warn('Invalid includeHidden type', {
      includeHidden: args.includeHidden,
      type: typeof args.includeHidden,
    });
    return null;
  }

  const maxElements = args.maxElements ?? 100;
  if (typeof maxElements !== 'number' || maxElements < 1 || maxElements > 500) {
    logger.warn('Invalid maxElements, using default', {
      maxElements: args.maxElements,
    });
    return null;
  }

  logger.debug('Validation successful', {
    sessionId: args.sessionId,
    selector,
    includeHidden,
    maxElements,
  });

  return {
    sessionId: args.sessionId,
    selector,
    includeHidden,
    maxElements,
  };
}

export const extractInteractableTool: StrictBrowserMCPTool = {
  name: 'extractInteractable',
  description:
    'Extract interactive elements from web pages with precise CSS selectors for automation. Identifies buttons, inputs, links, and other interactive elements with accurate selectors, current state, and metadata. Uses TypeScript parsing for better reliability and debugging.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: {
        type: 'string',
        description:
          'CSS selector to limit the search scope. Defaults to "body" for full page scan. Examples: ".main-content", "#form-container", "[data-component=navigation]"',
      },
      includeHidden: {
        type: 'boolean',
        description:
          'Include elements that are currently hidden but potentially interactive. Useful for detecting elements that might become visible through user actions. Default: false',
      },
      maxElements: {
        type: 'number',
        description:
          'Maximum number of interactive elements to return (1-500). Higher values may impact performance. Default: 100',
      },
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    const validatedArgs = validateExtractInteractableArgs(args);
    if (!validatedArgs) {
      return createMCPErrorResponse(
        -32602,
        'Invalid arguments provided - check sessionId, selector, includeHidden, and maxElements parameter types',
        { toolName: 'extractInteractable', args },
        createId(),
      );
    }

    const { sessionId, selector, includeHidden, maxElements } = validatedArgs;

    logger.debug('Executing extractInteractable', {
      sessionId,
      selector,
      includeHidden,
      maxElements,
    });

    if (!executeScript) {
      return createMCPErrorResponse(
        -32603,
        'executeScript function is required for extractInteractable',
        { toolName: 'extractInteractable', args },
        createId(),
      );
    }

    try {
      // Extract HTML from page (simple approach like extractContent)
      const rawHtml = await extractHtmlFromPage(
        executeScript,
        sessionId,
        selector,
      );

      // Parse HTML to find interactive elements (in TypeScript, not browser)
      const options: InteractableOptions = {
        includeHidden,
        maxElements,
      };

      const result = parseHtmlToInteractables(rawHtml, selector, options);

      if (result.error) {
        throw new Error(result.error);
      }

      const textContent = `Found ${result.metadata.total_count} interactive elements in ${result.metadata.performance.execution_time_ms}ms (${result.metadata.performance.data_size_bytes} bytes)`;

      logger.debug('extractInteractable completed successfully', {
        sessionId,
        selector,
        elementCount: result.metadata.total_count,
        executionTime: result.metadata.performance.execution_time_ms,
      });

      return createMCPStructuredResponse(
        textContent,
        result as unknown as Record<string, unknown>,
        createId(),
      );
    } catch (error) {
      logger.error('Error in extractInteractable:', {
        error,
        sessionId,
        selector,
        includeHidden,
        maxElements,
      });

      return createMCPErrorResponse(
        -32603,
        `Failed to extract interactive elements: ${error instanceof Error ? error.message : String(error)}`,
        { toolName: 'extractInteractable', args },
        createId(),
      );
    }
  },
};
