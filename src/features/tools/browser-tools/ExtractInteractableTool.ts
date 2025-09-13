import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { parseHtmlToInteractables } from '@/lib/html-parser';
import type {
  InteractableOptions,
  InteractableResult,
  InteractableElement,
} from '@/lib/html-parser';

const logger = getLogger('ExtractInteractableTool');

interface ValidatedArgs {
  sessionId: string;
  selector: string;
  includeHidden: boolean;
  maxElements: number;
}

// Element purpose estimation for better user experience
function estimateElementPurpose(el: InteractableElement): string {
  const text = el.text?.toLowerCase() || '';

  // Input field purpose estimation
  if (el.type === 'input') {
    if (el.inputType === 'email') return 'Email input field';
    if (el.inputType === 'password') return 'Password input field';
    if (el.inputType === 'search') return 'Search input field';
    if (el.inputType === 'tel') return 'Phone number input field';
    if (el.inputType === 'url') return 'URL input field';
    if (el.inputType === 'number') return 'Number input field';
    if (el.inputType === 'date') return 'Date input field';
    if (el.inputType === 'file') return 'File upload field';
    if (el.inputType === 'checkbox') return 'Checkbox';
    if (el.inputType === 'radio') return 'Radio button';
    if (el.placeholder) return `Input field (${el.placeholder})`;
    return 'Text input field';
  }

  // Button purpose estimation
  if (el.type === 'button') {
    if (text.includes('submit') || text.includes('send'))
      return 'Submit button';
    if (text.includes('login') || text.includes('sign in'))
      return 'Login button';
    if (text.includes('search')) return 'Search button';
    if (text.includes('cancel') || text.includes('close'))
      return 'Cancel/Close button';
    if (text.includes('save')) return 'Save button';
    if (text.includes('delete') || text.includes('remove'))
      return 'Delete button';
    if (text.includes('edit')) return 'Edit button';
    if (text.includes('next')) return 'Next button';
    if (
      text.includes('previous') ||
      text.includes('prev') ||
      text.includes('back')
    )
      return 'Previous/Back button';
    if (el.text) return `Button: "${el.text}"`;
    return 'Button (no text)';
  }

  // Link purpose estimation
  if (el.type === 'link') {
    if (text.includes('home')) return 'Home link';
    if (text.includes('about')) return 'About link';
    if (text.includes('contact')) return 'Contact link';
    if (text.includes('login') || text.includes('sign in')) return 'Login link';
    if (text.includes('register') || text.includes('sign up'))
      return 'Register link';
    if (text.includes('forgot') || text.includes('reset'))
      return 'Password reset link';
    if (el.text) return `Link: "${el.text}"`;
    return 'Link (no text)';
  }

  // Select dropdown
  if (el.type === 'select') {
    if (el.text) return `Dropdown: ${el.text}`;
    return 'Dropdown menu';
  }

  // Textarea
  if (el.type === 'textarea') {
    if (el.placeholder) return `Text area (${el.placeholder})`;
    return 'Text area';
  }

  // Fallback
  if (el.text) return `${el.type}: "${el.text}"`;
  return `${el.type} element`;
}

// Generate user-friendly text response
function generateInteractableText(result: InteractableResult): string {
  if (result.metadata.total_count === 0) {
    return 'No interactive elements found on this page.';
  }

  let text = `Found ${result.metadata.total_count} interactive elements:\n\n`;

  // Show first 20 elements to avoid overwhelming output
  const displayElements = result.elements.slice(0, 20);

  displayElements.forEach((el, index) => {
    const purpose = estimateElementPurpose(el);

    text += `${index + 1}. [${el.type.toUpperCase()}] ${purpose}\n`;
    text += `   Selector: ${el.selector}\n`;

    if (el.text && el.text.length <= 100) {
      text += `   Text: "${el.text}"\n`;
    } else if (el.text && el.text.length > 100) {
      text += `   Text: "${el.text.substring(0, 100)}..."\n`;
    }

    if (el.type === 'input' && el.placeholder && el.placeholder !== el.text) {
      text += `   Placeholder: "${el.placeholder}"\n`;
    }

    if (el.type === 'input' && el.value) {
      const displayValue =
        el.value.length <= 50 ? el.value : `${el.value.substring(0, 50)}...`;
      text += `   Current value: "${displayValue}"\n`;
    }

    const statusInfo: string[] = [];
    if (!el.enabled) statusInfo.push('DISABLED');
    if (!el.visible) statusInfo.push('HIDDEN');

    if (statusInfo.length > 0) {
      text += `   Status: ${statusInfo.join(', ')}\n`;
    }

    text += '\n';
  });

  // Show summary if there are more elements
  if (result.metadata.total_count > 20) {
    text += `... and ${result.metadata.total_count - 20} more elements (use maxElements parameter to see more)\n\n`;
  }

  text += `Performance: ${result.metadata.performance.execution_time_ms}ms`;
  return text;
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
        'Invalid arguments provided - check sessionId, selector, includeHidden, and maxElements parameter types',
        -32602,
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
        'executeScript function is required for extractInteractable',
        -32603,
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

      // Generate user-friendly text response
      const textContent = generateInteractableText(result);

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
        `Failed to extract interactable elements: ${error instanceof Error ? error.message : String(error)}`,
        -32603,
        { toolName: 'extractInteractable', args },
        createId(),
      );
    }
  },
};
