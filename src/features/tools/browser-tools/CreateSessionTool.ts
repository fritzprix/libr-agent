import { createBrowserSession } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictLocalMCPTool } from './types';

const logger = getLogger('CreateSessionTool');

export const createSessionTool: StrictLocalMCPTool = {
  name: 'createSession',
  description:
    'Creates a new interactive browser session in a separate window.',
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

    const sessionId = await createBrowserSession({
      url,
      title: title || null,
    });

    return createMCPTextResponse(
      `Browser session created successfully: ${sessionId}`,
    );
  },
};
