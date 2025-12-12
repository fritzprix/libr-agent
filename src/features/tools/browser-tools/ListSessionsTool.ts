import { listBrowserSessions } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
import { StrictLocalMCPTool } from './types';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('ListSessionsTool');

export const listSessionsTool: StrictLocalMCPTool = {
  name: 'listSessions',
  description:
    'Lists all active browser sessions with quick access to the current active session ID.',
  inputSchema: {
    type: 'object',
    properties: {},
    required: [],
  },
  execute: async () => {
    logger.debug('Executing browser_listSessions');
    const sessions = await listBrowserSessions();

    const currentActiveSessionId = sessions.length > 0 ? sessions[0].id : null;

    let message = '';
    if (sessions.length === 0) {
      message = 'No active browser sessions';
    } else if (sessions.length === 1) {
      message = `1 active browser session:\n\nSession ID: ${sessions[0].id}\nURL: ${sessions[0].url}\nTitle: ${sessions[0].title || 'N/A'}`;
    } else {
      message = `${sessions.length} active browser sessions:\n\n${sessions.map((s, i) => `${i + 1}. ID: ${s.id}\n   URL: ${s.url}\n   Title: ${s.title || 'N/A'}`).join('\n\n')}`;
    }

    return createMCPStructuredResponse(
      message,
      {
        sessions,
        currentActiveSessionId,
        count: sessions.length,
      },
      createId(),
    );
  },
};
