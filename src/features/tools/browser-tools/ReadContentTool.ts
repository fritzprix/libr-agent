import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { ContentStore } from './content-store';
import {
  validateSessionId,
  handleBrowserError,
  createBrowserErrorResponse,
} from './error-utils';

const logger = getLogger('ReadContentTool');

export const readWebContentTool: StrictBrowserMCPTool = {
  name: 'readWebContent',
  description:
    'Read a specific page of content extracted from a webpage. Use after calling extractWebContent.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      page: {
        type: 'number',
        description: 'The page number to read (1-based)',
      },
    },
    required: ['sessionId', 'page'],
  },
  execute: async (args: Record<string, unknown>) => {
    // Session validation
    const { isValid, errorResponse } = validateSessionId(args.sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    const sessionId = args.sessionId as string;
    const page =
      typeof args.page === 'number'
        ? args.page
        : parseInt(String(args.page), 10);

    if (isNaN(page) || page < 1) {
      return createBrowserErrorResponse(
        'Invalid arguments provided. Page must be a number >= 1.',
      );
    }

    logger.debug('Executing browser_readWebContent', {
      sessionId,
      page,
    });

    try {
      const contentPage = ContentStore.getPage(sessionId, page);

      if (!contentPage) {
        if (!ContentStore.hasContent(sessionId)) {
          return createBrowserErrorResponse(
            'No content found for this session. Please call extractWebContent first.',
          );
        }

        return createBrowserErrorResponse(`Page ${page} not found.`);
      }
      let responseText = contentPage.content;

      // 빈 페이지 감지 및 경고 메시지 추가
      if (!contentPage.content.trim()) {
        responseText += `\n\n(Empty Page) The extracted content is empty. This suggests the page might not have loaded correctly or contains no text. Please try calling 'extractWebContent' again to re-capture the page, or use 'extractWebContent' with 'saveRawHtml': true to save the raw HTML for inspection.`;
      }

      return createMCPStructuredResponse(
        responseText,
        {
          page: contentPage.pageNumber,
          total_pages: contentPage.totalPages,
          content_length: contentPage.content.length,
        },
        createId(),
      );
    } catch (error) {
      return handleBrowserError(error, {
        toolName: 'readWebContent',
        sessionId,
      });
    }
  },
};
