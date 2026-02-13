import { useState, useEffect, useCallback } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentChatActions } from '@/context/AgentChatContext';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { stringToMCPContentArray } from '@/lib/utils';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentWorkspacePanel');

export function useFileDrop(
  rootPath: string,
  onFileImported: () => void,
  panelRef: React.RefObject<HTMLDivElement>
) {
  const { agentCallBuiltinTool } = useRustBackend();
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { subscribe } = useDnDContext();
  const [dragState, setDragState] = useState<{ isOver: boolean }>({
    isOver: false,
  });

  // Handle external file drops from DnDContext
  const handleWorkspaceFileDrop = useCallback(
    async (paths: string[]) => {
      if (!session?.id) return;

      logger.info('External files dropped on workspace', {
        fileCount: paths.length,
        targetPath: rootPath,
      });

      try {
        for (const srcPath of paths) {
          // OS-agnostic path parsing: support both / and \\ separators
          const fileName = srcPath.split(/[/\\]/).pop() || 'unknown';
          const destPath = `${rootPath}/${fileName}`.replace(/\/+/g, '/');
          const destRelPath = destPath.startsWith('./')
            ? destPath.slice(2)
            : destPath;

          // Call builtin workspace tool (returns MCPResult directly after fix)
          const response = (await agentCallBuiltinTool(
            session.id,
            'builtin_workspace__importFile',
            {
              src_abs_path: srcPath,
              dest_rel_path: destRelPath,
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
                    typeof (item as Record<string, unknown>)['text'] ===
                      'string'
                  ) {
                    texts.push(
                      (item as Record<string, unknown>)['text'] as string,
                    );
                  } else if (
                    (item as Record<string, unknown>)['type'] === 'text' &&
                    !('text' in (item as Record<string, unknown>))
                  ) {
                    // explicit text type but missing text field - skip
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
              resultText = 'No result returned from importFile';
            }
          } catch (e) {
            resultText = `Failed to parse tool response: ${
              e instanceof Error ? e.message : String(e)
            }`;
          }

          const [toolCallMessage, toolResultMessage] = createToolMessagePair(
            'builtin_workspace__importFile',
            { src_abs_path: srcPath, dest_rel_path: destRelPath },
            stringToMCPContentArray(resultText),
            toolCallId,
            session.id,
            undefined,
            session.assistant?.id,
            'ui',
          );

          // Submit messages atomically using injectMessages
          await injectMessages([toolCallMessage, toolResultMessage], true);
        }

        // Refresh directory after import
        onFileImported();
      } catch (error) {
        logger.error('File import failed', error);
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error('Failed to import file', {
          description: message,
        });
      }
    },
    [agentCallBuiltinTool, injectMessages, session, rootPath, onFileImported]
  );

  // Subscribe to DnD events
  useEffect(() => {
    logger.debug('Setting up DnD subscription for AgentWorkspacePanel');

    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      logger.debug('DnD event received in AgentWorkspacePanel', {
        event,
        paths: payload.paths,
      });

      if (event === 'drag-over') {
        setDragState({ isOver: true });
      } else if (event === 'drop') {
        setDragState({ isOver: false });
        if (payload.paths) {
          handleWorkspaceFileDrop(payload.paths);
        }
      } else if (event === 'leave') {
        setDragState({ isOver: false });
      }
    };

    const unsub = subscribe(panelRef, handler, { priority: 5 });

    return () => {
      logger.debug('Cleaning up DnD subscription for AgentWorkspacePanel');
      unsub();
    };
  }, [subscribe, handleWorkspaceFileDrop, panelRef]);

  return {
    dragState,
  };
}
