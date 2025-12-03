import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { ContentStore } from './content-store';

const logger = getLogger('ReadContentTool');

interface ValidatedArgs {
  sessionId: string;
  page: number;
}

function validateReadContentArgs(
  args: Record<string, unknown>,
): ValidatedArgs | null {
  if (typeof args.sessionId !== 'string') {
    return null;
  }

  const page =
    typeof args.page === 'number' ? args.page : parseInt(String(args.page), 10);
  if (isNaN(page) || page < 1) {
    return null;
  }

  return {
    sessionId: args.sessionId,
    page,
  };
}

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
    const validatedArgs = validateReadContentArgs(args);
    if (!validatedArgs) {
      return createMCPErrorResponse(
        'Invalid arguments provided. Page must be a number >= 1.',
        -32602,
        { toolName: 'readWebContent', args },
        createId(),
      );
    }

    const { sessionId, page } = validatedArgs;

    logger.debug('Executing browser_readWebContent', {
      sessionId,
      page,
    });

    const contentPage = ContentStore.getPage(sessionId, page);

    if (!contentPage) {
      if (!ContentStore.hasContent(sessionId)) {
        return createMCPErrorResponse(
          'No content found for this session. Please call extractWebContent first.',
          -32604, // Content not found
          { toolName: 'readWebContent', args },
          createId(),
        );
      }

      return createMCPErrorResponse(
        `Page ${page} not found.`,
        -32604,
        { toolName: 'readWebContent', args },
        createId(),
      );
    }

    const responseText = `[Page ${contentPage.pageNumber}/${contentPage.totalPages}]\n\n${contentPage.content}`;

    return createMCPStructuredResponse(
      responseText,
      {
        page: contentPage.pageNumber,
        total_pages: contentPage.totalPages,
        content_length: contentPage.content.length,
      },
      createId(),
    );
  },
};
