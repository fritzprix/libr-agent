import { useCallback } from 'react';
import type { MCPContent } from '@/lib/mcp';
import { extractServiceInfoFromContent } from '@/lib/mcp';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import { UIActionResult } from '@mcp-ui/client';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  createSystemMessage,
  createUserMessage,
  createToolMessagePair,
} from '@/lib/chat-utils';
import { handleUserToolCall } from '@/lib/backend';
import { createId } from '@paralleldrive/cuid2';
import { isBuiltinTool } from '@/lib/tool-call-utils';

const logger = getLogger('AgentMessageRenderer');

/**
 * Handle UI Action from UIResourceRenderer
 *
 * V2 Simplified Logic:
 * - Tool execution only, message pair creation handled by Rust
 * - UI Resource detection handled by Rust (hasToolCall && !hasUIResource)
 * - Frontend only receives results via agent:event
 */
export function useUIActionHandler(
  contentRef: React.MutableRefObject<MCPContent[]>,
) {
  const { session } = useAgentSessionState();
  const { submit, injectMessages } = useAgentChatActions();
  const tauriCommands = useRustBackend();
  const { openExternalUrl } = tauriCommands;

  return useCallback(
    async (result: UIActionResult) => {
      const sessionId = session?.id;

      if (!sessionId) {
        logger.warn('No active session for UI action', { type: result.type });
        return;
      }

      try {
        switch (result.type) {
          case 'tool': {
            const { toolName, params = {} } = result.payload;
            logger.info('UI Action Tool Call Received', {
              sessionId,
              result,
            });

            // prefix routing: tauri: prefix means internal Tauri command
            if (toolName.startsWith('tauri:')) {
              const [, strippedCommand] = toolName.split('tauri:');

              // Check if method exists in tauriCommands
              if (
                strippedCommand &&
                typeof tauriCommands[
                  strippedCommand as keyof typeof tauriCommands
                ] === 'function'
              ) {
                try {
                  let resultText: string;

                  // Explicit handling for each Tauri command
                  switch (strippedCommand) {
                    case 'downloadWorkspaceFile': {
                      resultText = await tauriCommands.downloadWorkspaceFile(
                        params.filePath as string,
                        sessionId,
                      );
                      break;
                    }
                    case 'downloadMediaFile': {
                      resultText = await tauriCommands.downloadMediaFile({
                        sessionId,
                        fileName: params.fileName as string | undefined,
                        mimeType: params.mimeType as string,
                        dataBase64: params.dataBase64 as string | undefined,
                      });
                      break;
                    }
                    case 'exportAndDownloadZip': {
                      resultText = await tauriCommands.exportAndDownloadZip(
                        params.files as string[],
                        params.packageName as string,
                        sessionId,
                      );
                      break;
                    }
                    case 'openExternalUrl': {
                      await tauriCommands.openExternalUrl(params.url as string);
                      resultText = 'External URL opened successfully';
                      break;
                    }
                    default: {
                      throw new Error(
                        `Unsupported Tauri command: ${strippedCommand}`,
                      );
                    }
                  }

                  logger.info('Tauri command executed', {
                    command: strippedCommand,
                    result: resultText,
                  });

                  // --- V2 Result Handling Fix ---
                  // Manually inject ToolCall and ToolResult to history and TRIGGER the workflow.
                  // This allows the Agent to see the file action and respond (recursion).

                  // 1. Create a unique tool call ID
                  const toolCallId = createId();

                  // 2. Create the message pair (Call + Result)
                  const [toolCallMsg, toolResultMsg] = createToolMessagePair(
                    toolName, // Use full name e.g. "tauri:downloadWorkspaceFile"
                    params,
                    [{ type: 'text', text: resultText }],
                    toolCallId,
                    sessionId,
                    undefined,
                    session.assistant?.id, // assistantId
                    'ui',
                  );

                  // 3. Inject both and trigger workflow
                  // "triggerWorkflow: true" manually calls request_llm_completion
                  await injectMessages([toolCallMsg, toolResultMsg], true);
                } catch (error) {
                  logger.error('Tauri command failed', {
                    command: strippedCommand,
                    error,
                  });

                  // Optional: Inject failure message if needed, or just toast
                  // For now, let's inject a failure result to keep history consistent
                  const toolCallId = createId();
                  const errorMsg =
                    error instanceof Error ? error.message : String(error);
                  const [toolCallMsg, toolResultMsg] = createToolMessagePair(
                    toolName,
                    params,
                    [{ type: 'text', text: `Error: ${errorMsg}` }],
                    toolCallId,
                    sessionId,
                    undefined,
                    session.assistant?.id,
                    'ui',
                  );
                  // Still trigger workflow so agent knows it failed? Or maybe not?
                  // Agentic philosophy: Agent should know it failed.
                  await injectMessages([toolCallMsg, toolResultMsg], true);
                }
              } else {
                logger.warn('Tauri command not found', {
                  command: strippedCommand,
                  availableMethods: Object.keys(tauriCommands),
                });
              }
              return { status: 'tauri-processed' };
            } else {
              // MCP tool call: extract service info from latest content
              const serviceInfo = extractServiceInfoFromContent(
                contentRef.current,
              );

              let finalToolName = toolName;
              if (serviceInfo) {
                const isBaseName =
                  !toolName.includes('__') && !isBuiltinTool(toolName);

                logger.debug('UI Action Tool Call - Name Resolution', {
                  originalToolName: toolName,
                  isBaseName,
                  backendType: serviceInfo.backendType,
                  serverName: serviceInfo.serverName,
                });

                if (isBaseName) {
                  // All tools (both builtin and external) use the same format: server__tool
                  finalToolName = `${serviceInfo.serverName}__${toolName}`;
                }
              } else {
                logger.warn(
                  'No service context available, using original tool name',
                  {
                    toolName,
                  },
                );
              }

              // Unified MCP tool call (V2: Rust Single Backend)
              logger.info(
                'Injecting Tool Call via Rust Backend (Assistant Role)',
                {
                  sessionId,
                  toolName: finalToolName,
                },
              );

              // Use type-safe wrapper to handle the tool call as an Assistant message
              // This triggers the Rust backend to execute the tool and resume the workflow automatically
              await handleUserToolCall(sessionId, finalToolName, params);

              return { status: 'tool-submitted', tool: finalToolName };
            }
          }

          case 'intent': {
            // Convert intent to natural language prompt
            const intentText = `User intent: ${result.payload.intent}`;
            const paramsText = result.payload.params
              ? `\nParameters: ${JSON.stringify(result.payload.params, null, 2)}`
              : '';

            const intentMessage = createUserMessage(
              intentText + paramsText,
              sessionId,
              undefined, // assistantId bound to session
              'ui',
            );

            await submit(intentMessage);
            return {
              status: 'intent-submitted',
              intent: result.payload.intent,
            };
          }

          case 'prompt': {
            const promptMessage = createUserMessage(
              result.payload.prompt,
              sessionId,
              undefined, // assistantId bound to session
              'ui',
            );

            await submit(promptMessage);
            return { status: 'prompt-submitted' };
          }

          case 'link': {
            await openExternalUrl(result.payload.url);
            return { status: 'link-opened' };
          }

          case 'notify': {
            // Add notification as system message
            const notificationMessage = createSystemMessage(
              `[Notification] ${result.payload.message}`,
              sessionId,
              undefined, // assistantId bound to session
              'ui',
            );

            await submit(notificationMessage);
            return { status: 'notified' };
          }

          default: {
            logger.warn('Unknown UI action type', {
              type: (result as { type: string }).type,
              result,
            });
            return { status: 'unknown-action' };
          }
        }
      } catch (error) {
        logger.error('Failed to handle UI action', {
          type: result.type,
          error: error instanceof Error ? error.message : String(error),
        });
        return {
          status: 'error',
          message: error instanceof Error ? error.message : String(error),
        };
      }
    },
    [
      session?.id,
      session?.assistant?.id,
      submit,
      openExternalUrl,
      tauriCommands,
      injectMessages,
      contentRef,
    ],
  );
}
