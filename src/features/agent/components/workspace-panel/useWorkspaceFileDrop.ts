import { useCallback } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import { join } from '@tauri-apps/api/path';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { stringToMCPContentArray } from '@/lib/utils';

const logger = getLogger('useWorkspaceFileDrop');

export function useWorkspaceFileDrop(
  rootPath: string,
  onDropComplete: () => void,
) {
  const { t } = useTranslation();
  const { agentCallBuiltinTool } = useRustBackend();
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();

  const handleWorkspaceFileDrop = useCallback(
    async (paths: string[]) => {
      if (!session?.id || paths.length === 0) return;

      logger.info('External files dropped on workspace', {
        fileCount: paths.length,
        targetPath: rootPath,
      });

      try {
        const filesToImport = await Promise.all(
          paths.map(async (srcPath) => {
            const fileName = srcPath.split(/[/\\]/).pop() || 'unknown';
            const destPath = await join(rootPath, fileName);
            let destRelPath = destPath.replace(/\\/g, '/');
            if (destRelPath.startsWith('./')) {
              destRelPath = destRelPath.slice(2);
            }
            return {
              srcAbsPath: srcPath,
              destRelPath,
            };
          }),
        );

        // Call builtin workspace tool for batch import
        const response = (await agentCallBuiltinTool(
          session.id,
          'workspace__importFiles',
          {
            files: filesToImport,
          },
        )) as {
          content?: Array<{ type: string; text?: string }>;
          structuredContent?: unknown;
          isError?: boolean;
        };

        // Create tool messages for chat history
        const toolCallId = createId();

        // Build a safe textual result for UI.
        let resultText = '';

        try {
          // Check if this is an error result
          if (response.isError === true) {
            // Extract error message from content
            const errorContent = response.content?.[0];
            if (
              errorContent &&
              typeof errorContent === 'object' &&
              'text' in errorContent
            ) {
              resultText = `${errorContent.text}`;
            } else {
              resultText = 'Tool execution failed';
            }
          } else if (response.content && Array.isArray(response.content)) {
            // Extract text from content array
            const texts: string[] = [];
            for (const item of response.content) {
              if (item && typeof item === 'object') {
                if (
                  'text' in (item as Record<string, unknown>) &&
                  typeof (item as Record<string, unknown>)['text'] === 'string'
                ) {
                  texts.push(
                    (item as Record<string, unknown>)['text'] as string,
                  );
                } else {
                  try {
                    texts.push(JSON.stringify(item));
                  } catch {
                    // ignore
                  }
                }
              }
            }

            if (texts.length > 0) resultText = texts.join('\n');
            else resultText = JSON.stringify(response.content);
          } else {
            resultText = 'No result returned from importFiles';
          }
        } catch (e) {
          resultText = `Failed to parse tool response: ${
            e instanceof Error ? e.message : String(e)
          }`;
        }

        const [toolCallMessage, toolResultMessage] = createToolMessagePair(
          'workspace__importFiles',
          { files: filesToImport },
          stringToMCPContentArray(resultText),
          toolCallId,
          session.id,
          undefined,
          session.assistant?.id,
          'ui',
        );

        // Submit messages atomically using injectMessages
        await injectMessages([toolCallMessage, toolResultMessage]);

        // Refresh directory after import
        onDropComplete();
      } catch (error) {
        logger.error('File import failed', error);
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error(t('agent.workspace.importFileError'), {
          description: message,
        });
      }
    },
    [
      agentCallBuiltinTool,
      injectMessages,
      session,
      rootPath,
      onDropComplete,
      t,
    ],
  );

  return { handleWorkspaceFileDrop };
}
