import { useCallback, useEffect } from 'react';
import { useChatContext } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import { useBuiltInTool } from '@/features/tools';
import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BuiltInToolsSystemPrompt');

/**
 * System prompt component that injects available tools and attached files
 * into the chat context for AI awareness.
 */
export function BuiltInToolsSystemPrompt() {
  const { registerSystemPrompt, unregisterSystemPrompt } = useChatContext();
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');
  const { getCurrentSession } = useSessionContext();
  const { availableTools } = useBuiltInTool();
  const isLoadingTauriTools = false; // No longer tracked in new API

  const buildPrompt = useCallback(async () => {
    let promptSections = [];

    // 1. Built-in Tools Section
    promptSections.push(`# Available Built-in Tools

You have access to built-in tools for file operations, code execution, and web-based processing.
Tool details and usage instructions are provided separately.

**Available Built-In Tools:** ${availableTools.length} ${isLoadingTauriTools ? '(Loading...)' : ''}

**Important Instruction:** When calling built-in tools, you MUST use the tool name exactly as it appears in the available tools list. Do not add or remove the "builtin." prefix - use it "as is" (e.g., if the tool name is "builtin.file_read", call it as "builtin.file_read", not "file_read" or "builtin.builtin.file_read").
`);

    // 2. Attached Files Section
    const currentSession = getCurrentSession();
    if (!currentSession?.storeId) {
      promptSections.push('\n# Attached Files\nNo files currently attached.');
    } else {
      try {
        const result = await server?.listContent({
          storeId: currentSession.storeId,
        });

        if (!result?.contents || result.contents.length === 0) {
          promptSections.push(
            '\n# Attached Files\nNo files currently attached.',
          );
        } else {
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

          promptSections.push(`\n# Attached Files\n${attachedResources}`);

          logger.debug('Built attached files prompt', {
            sessionId: currentSession.id,
            fileCount: result.contents.length,
          });
        }
      } catch (error) {
        logger.error('Failed to build attached files prompt', {
          sessionId: currentSession.id,
          error: error instanceof Error ? error.message : String(error),
        });
        promptSections.push(
          '\n# Attached Files\nError loading attached files.',
        );
      }
    }

    const fullPrompt = promptSections.join('\n');

    return fullPrompt;
  }, [getCurrentSession, server, availableTools.length, isLoadingTauriTools]);

  useEffect(() => {
    // Register the system prompt even if server is not available yet
    // Built-in tools should always be available
    const id = registerSystemPrompt({
      content: buildPrompt,
      priority: 1,
    });

    logger.debug('Registered built-in tools system prompt', {
      promptId: id,
      toolCount: availableTools.length,
      hasServer: !!server,
    });

    return () => {
      unregisterSystemPrompt(id);
      logger.debug('Unregistered built-in tools system prompt', {
        promptId: id,
      });
    };
  }, [
    buildPrompt,
    registerSystemPrompt,
    unregisterSystemPrompt,
    availableTools.length,
    server,
  ]);

  return null;
}
