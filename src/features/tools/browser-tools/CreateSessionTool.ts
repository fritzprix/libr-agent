import { createBrowserSession } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { LocalMCPTool } from './types';

const logger = getLogger('CreateSessionTool');

export const createSessionTool: LocalMCPTool = {
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

    return `Browser session created successfully: ${sessionId}`;
  },
};
