import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { parseHtmlToInteractables } from '@/lib/html-parser';
import type {
  InteractableOptions,
  InteractableResult,
  InteractableElement,
} from '@/lib/html-parser';
import {
  validateSessionId,
  handleBrowserError,
  createBrowserErrorResponse,
} from './error-utils';

const logger = getLogger('ExtractInteractableTool');

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
function generateInteractableText(
  result: InteractableResult,
  page: number = 1,
  pageSize: number = 20,
): string {
  if (result.metadata.total_count === 0) {
    return 'No interactive elements found on this page.';
  }

  const total = result.metadata.total_count;
  const totalPages = Math.ceil(total / pageSize);
  const start = (page - 1) * pageSize;
  const end = start + pageSize;
  const displayElements = result.elements.slice(start, end);

  let text = `Found ${total} interactive elements (Page ${page}/${totalPages}):\n\n`;

  if (displayElements.length === 0) {
    return text + 'No elements on this page.';
  }

  displayElements.forEach((el, index) => {
    const purpose = estimateElementPurpose(el);

    text += `${start + index + 1}. [${el.type.toUpperCase()}] ${purpose}\n`;
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
  if (page < totalPages) {
    text += `... and ${total - end} more elements. Use page=${page + 1} to see more.\n\n`;
  }

  text += `Performance: ${result.metadata.performance.execution_time_ms}ms`;
  return text;
}

// Simple HTML extraction function (following extractContent pattern)
async function extractHtmlFromPage(
  executeScript: (sessionId: string, script: string) => Promise<unknown>,
  sessionId: string,
): Promise<string> {
  const rawHtml = await executeScript(
    sessionId,
    `document.documentElement.outerHTML`,
  );

  if (!rawHtml || typeof rawHtml !== 'string') {
    throw new Error(
      'Failed to extract HTML from the page - no content found or invalid content type',
    );
  }

  return rawHtml;
}

export const listInteractableTool: StrictBrowserMCPTool = {
  name: 'listInteractable',
  description:
    'List all interactive elements from the entire web page for automation. Identifies buttons, inputs, links, and other interactive elements with accurate selectors, current state, and metadata. Uses TypeScript parsing for better reliability and debugging. Supports pagination for large lists.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
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
      page: {
        type: 'number',
        description: 'Page number for the text summary (default: 1)',
      },
      pageSize: {
        type: 'number',
        description:
          'Number of elements per page for the text summary (default: 20)',
      },
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    // Session validation
    const { isValid, errorResponse } = validateSessionId(args.sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    const sessionId = args.sessionId as string;
    const includeHidden =
      typeof args.includeHidden === 'boolean' ? args.includeHidden : false;
    const maxElements =
      typeof args.maxElements === 'number' ? args.maxElements : 100;
    const page = typeof args.page === 'number' ? args.page : 1;
    const pageSize = typeof args.pageSize === 'number' ? args.pageSize : 20;

    logger.debug('Executing listInteractable', {
      sessionId,
      includeHidden,
      maxElements,
      page,
      pageSize,
    });

    if (!executeScript) {
      return createBrowserErrorResponse(
        '✗ List failed: executeScript function is required',
      );
    }

    try {
      // Extract HTML from page (simple approach like extractContent)
      const rawHtml = await extractHtmlFromPage(executeScript, sessionId);

      // Parse HTML to find interactive elements (in TypeScript, not browser)
      const options: InteractableOptions = {
        includeHidden,
        maxElements,
      };

      const result = parseHtmlToInteractables(rawHtml, 'body', options);

      if (result.error) {
        throw new Error(result.error);
      }

      // Generate user-friendly text response
      const textContent = generateInteractableText(result);

      logger.debug('listInteractable completed successfully', {
        sessionId,
        elementCount: result.metadata.total_count,
        executionTime: result.metadata.performance.execution_time_ms,
      });

      return createMCPStructuredResponse(
        textContent,
        result as unknown as Record<string, unknown>,
        createId(),
      );
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'listInteractable',
        sessionId,
      });
    }
  },
};
