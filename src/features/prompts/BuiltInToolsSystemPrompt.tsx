import { useCallback, useEffect } from 'react';
import { useChatContext } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useWebMCPServer } from '@/context/WebMCPContext';
import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BuiltInToolsSystemPrompt');

/**
 * System prompt component that dynamically injects information about attached files
 * into the chat context. This allows the AI to be aware of files that have been
 * uploaded to the current session's content store.
 *
 * This component automatically registers a system prompt extension that:
 * - Lists all files attached to the current session
 * - Provides file metadata (name, type, size, preview)
 * - Updates when files are added/removed from the session
 * - Handles errors gracefully with fallback messages
 */
export function BuiltInToolsSystemPrompt() {
  const { registerSystemPrompt, unregisterSystemPrompt } = useChatContext();
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');
  const { getCurrentSession } = useSessionContext();

  const buildPrompt = useCallback(async () => {
    const currentSession = getCurrentSession();
    if (!currentSession?.storeId) {
      logger.warn(
        'No current session available for building attached files prompt',
      );
      return '# Attached Files\nNo files currently attached.';
    }

    try {
      const result = await server?.listContent({
        storeId: currentSession.storeId,
      });

      if (!result?.contents || result.contents.length === 0) {
        return '# Attached Files\nNo files currently attached.';
      }

      const attachedResources = result.contents
        .map((c) =>
          JSON.stringify({
            storeId: c.storeId,
            contentId: c.contentId,
            preview: c.preview,
            filename: c.filename,
            type: c.mimeType,
            size: c.size,
          }),
        )
        .join('\n');

      logger.debug('Built attached files prompt', {
        sessionId: currentSession.id,
        fileCount: result.contents.length,
      });

      return `# Attached Files\n${attachedResources}`;
    } catch (error) {
      logger.error('Failed to build attached files prompt', {
        sessionId: currentSession.id,
        error: error instanceof Error ? error.message : String(error),
      });
      return '# Attached Files\nError loading attached files.';
    }
  }, [getCurrentSession, server]);

  useEffect(() => {
    if (!server) {
      logger.debug(
        'Skipping system prompt registration - missing session or server',
      );
      return;
    }

    const id = registerSystemPrompt({
      content: buildPrompt,
      priority: 1,
    });

    return () => {
      unregisterSystemPrompt(id);
      logger.debug('Unregistered attached files system prompt', {
        promptId: id,
      });
    };
  }, [server, buildPrompt, registerSystemPrompt, unregisterSystemPrompt]);

  return null;
}
