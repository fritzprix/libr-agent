import {
  createBrowserSession,
  listBrowserSessions,
} from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictLocalMCPTool } from './types';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('CreateSessionTool');

export const createSessionTool: StrictLocalMCPTool = {
  name: 'createSession',
  description:
    'Creates a new interactive browser session in a separate window. If a session already exists, returns the existing session information instead of creating a duplicate.',
  inputSchema: {
    type: 'object',
    properties: {
      url: BROWSER_TOOL_SCHEMAS.url,
      title: BROWSER_TOOL_SCHEMAS.title,
    },
    required: ['url'],
  },
  execute: async (args: Record<string, unknown>) => {
    const { url, title } = args as { url: string; title?: string };
    logger.debug('Executing browser_createSession', { url, title });

    const existingSessions = await listBrowserSessions();

    if (existingSessions.length > 0) {
      const session = existingSessions[0];
      logger.info('Active session already exists, returning existing session', {
        sessionId: session.id,
      });

      return createMCPStructuredResponse(
        `⚠️ Active session already exists\n\nSession ID: ${session.id}\nCurrent URL: ${session.url}\nTitle: ${session.title || 'N/A'}\n\nUse this session ID for browser operations. If you need to navigate to a different URL, use navigateToUrl.`,
        {
          sessionId: session.id,
          url: session.url,
          title: session.title,
          wasExisting: true,
        },
        createId(),
      );
    }

    const sessionId = await createBrowserSession({
      url,
      title: title || null,
    });

    return createMCPStructuredResponse(
      `✓ Browser session created successfully\n\nSession ID: ${sessionId}\nURL: ${url}`,
      {
        sessionId,
        url,
        title,
        wasExisting: false,
      },
      createId(),
    );
  },
};
