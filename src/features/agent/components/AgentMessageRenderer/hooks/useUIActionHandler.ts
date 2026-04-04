import { useCallback } from 'react';
import type { MCPContent } from '@/lib/mcp';
import { extractServiceInfoFromContent } from '@/lib/mcp';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import { UIActionResult } from '@mcp-ui/client';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { createSystemMessage, createUserMessage } from '@/lib/chat-utils';
import { executeUiTauriAction, handleUserToolCall } from '@/lib/backend';
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
  const { submit } = useAgentChatActions();
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
              const response = await executeUiTauriAction(
                sessionId,
                toolName,
                params,
              );

              return {
                status: response.success ? 'tauri-processed' : 'tauri-error',
                message: response.message,
              };
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
    [session?.id, submit, openExternalUrl, contentRef],
  );
}
