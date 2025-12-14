import {
  createBrowserSession,
  listBrowserSessions,
  navigateToUrl,
} from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS, getNavigationHint } from './helpers';
import { StrictLocalMCPTool } from './types';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('CreateSessionTool');

export const createSessionTool: StrictLocalMCPTool = {
  name: 'createSession',
  description:
    'Creates a new interactive browser session in a separate window. NOTE: Only one active session is supported at a time. If a session already exists, this tool will reuse it and navigate to the specified URL if different.',
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
      logger.info('Active session already exists, reusing session', {
        sessionId: session.id,
        currentUrl: session.url,
        targetUrl: url,
      });

      let message = `⚠️ Active session already exists (ID: ${session.id})\n`;
      let navigated = false;

      if (session.url !== url) {
        try {
          await navigateToUrl(session.id, url);
          message += `✓ Navigated to new URL: ${url}\n`;
          navigated = true;
        } catch (error) {
          message += `✗ Failed to navigate to ${url}: ${error}\n`;
        }
      } else {
        message += `✓ Already at requested URL: ${url}\n`;
      }

      message += `Title: ${session.title || 'N/A'}\n\nUse this session ID for browser operations.`;

      return createMCPStructuredResponse(
        message,
        {
          sessionId: session.id,
          url: url,
          title: session.title,
          wasExisting: true,
          navigated,
        },
        createId(),
      );
    }

    const { session_id: sessionId, message } = await createBrowserSession({
      url,
      title: title || null,
    });

    return createMCPStructuredResponse(
      `✓ Browser session created successfully\n\nSession ID: ${sessionId}\nURL: ${url}${getNavigationHint(message)}`,
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
