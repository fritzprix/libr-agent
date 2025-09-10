import { navigateToUrl } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictLocalMCPTool } from './types';

const logger = getLogger('NavigateToUrlTool');

export const navigateToUrlTool: StrictLocalMCPTool = {
  name: 'navigateToUrl',
  description: 'Navigates to a new URL in an existing browser session.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      url: BROWSER_TOOL_SCHEMAS.url,
    },
    required: ['sessionId', 'url'],
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId, url } = args as {
      sessionId: string;
      url: string;
    };
    logger.debug('Executing browser_navigateToUrl', { sessionId, url });

    const result = await navigateToUrl(sessionId, url);
    return createMCPTextResponse(result);
  },
};
