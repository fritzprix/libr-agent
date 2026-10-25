import { useCallback } from 'react';
import type { MCPContent } from '@/lib/mcp';
import { extractServiceInfoFromContent } from '@/lib/mcp';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useIsDarkMode } from '@/hooks/use-is-dark-mode';
import { getLogger } from '@/lib/logger';
import { UIActionResult } from '@mcp-ui/client';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { createSystemMessage, createUserMessage } from '@/lib/chat-utils';
import { executeUiTauriAction, handleUserToolCall } from '@/lib/backend';
import { isBuiltinTool } from '@/lib/tool-call-utils';
import { isWorkflowCancelledError } from '@/context/llm/types';
import {
  exportMarkdownDocumentWithNotify,
  markdownExportBaseName,
  markdownExportLabelsFromT,
} from '@/features/agent/lib/markdown-document-export';
import { useTranslation } from 'react-i18next';

const logger = getLogger('AgentMessageRenderer');

/** In-flight interactive callback keys — blocks duplicate form submits. */
const inFlightUiCallbacks = new Set<string>();

function readStringParam(
  params: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = params[key];
  return typeof value === 'string' ? value : undefined;
}

function isInteractiveUiCallback(toolName: string): boolean {
  const base = toolName.includes('__')
    ? toolName.slice(toolName.lastIndexOf('__') + 2)
    : toolName;
  return base === 'getUserAnswer' || base === 'resumeCircuitBreak';
}

function interactiveCallbackKey(
  toolName: string,
  params: Record<string, unknown>,
): string {
  const messageId = readStringParam(params, 'messageId');
  if (messageId) {
    return `${toolName}:${messageId}`;
  }
  return `${toolName}:${JSON.stringify(params)}`;
}

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
  messageSessionId?: string,
) {
  const { session } = useAgentSessionState();
  const { submit } = useAgentChatActions();
  const tauriCommands = useRustBackend();
  const { openExternalUrl } = tauriCommands;
  const isDark = useIsDarkMode();
  const { t } = useTranslation('common');

  return useCallback(
    async (result: UIActionResult) => {
      // Prefer live session context; fall back to the message's session when
      // context briefly lags after resume / session switch.
      const sessionId = session?.id ?? messageSessionId;

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

            // Frontend-owned markdown export (Mermaid/LaTeX PDF preprocess).
            // presentInteractive posts these instead of raw downloadMediaFile
            // so export matches ReportResultCard / MessageActionBar.
            if (
              toolName === 'tauri:exportMarkdownFile' ||
              toolName === 'tauri:exportMarkdownPdf'
            ) {
              const content = readStringParam(params, 'content') ?? '';
              if (!content.trim()) {
                const { toast } = await import('sonner');
                toast.error(t('agent.bubble.actionBar.copyEmpty'));
                return {
                  status: 'tauri-processed',
                  message: `UI export skipped (empty): ${toolName}`,
                };
              }

              const fileBaseName = markdownExportBaseName(
                readStringParam(params, 'fileBaseName') ??
                  readStringParam(params, 'fileName'),
                'export',
              );
              const kind =
                toolName === 'tauri:exportMarkdownPdf' ? 'pdf' : 'markdown';

              await exportMarkdownDocumentWithNotify({
                content,
                kind,
                fileBaseName,
                isDark,
                labels: markdownExportLabelsFromT(t),
              });

              return {
                status: 'tauri-processed',
                message: `UI export executed: ${toolName}`,
              };
            }

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

              const interactive = isInteractiveUiCallback(finalToolName);
              const callbackKey = interactive
                ? interactiveCallbackKey(finalToolName, params)
                : null;

              if (callbackKey && inFlightUiCallbacks.has(callbackKey)) {
                logger.info('Ignoring duplicate interactive UI callback', {
                  sessionId,
                  toolName: finalToolName,
                  callbackKey,
                });
                // Keep iframe locked: treat as accepted so the user does not unlock
                // and re-submit into a circuit-breaker loop.
                return {
                  status: 'tool-submitted',
                  tool: finalToolName,
                  message: t('agent.interactiveForm.duplicateSubmit'),
                };
              }

              if (callbackKey) {
                inFlightUiCallbacks.add(callbackKey);
              }

              try {
                // Use type-safe wrapper to handle the tool call as an Assistant message
                // This triggers the Rust backend to execute the tool and resume the workflow automatically
                await handleUserToolCall(sessionId, finalToolName, params);

                if (interactive) {
                  const { toast } = await import('sonner');
                  toast.success(t('agent.interactiveForm.submitSuccess'));
                  return {
                    status: 'tool-submitted',
                    tool: finalToolName,
                    message: t('agent.interactiveForm.submitSuccess'),
                  };
                }

                return { status: 'tool-submitted', tool: finalToolName };
              } catch (error) {
                const errorMessage =
                  error instanceof Error ? error.message : String(error);

                if (interactive && isWorkflowCancelledError(errorMessage)) {
                  const { toast } = await import('sonner');
                  toast.error(t('agent.interactiveForm.submitNotReady'));
                  // Surface as error (not ignored) so the iframe unlocks for retry.
                  return {
                    status: 'error',
                    message: t('agent.interactiveForm.submitNotReady'),
                  };
                }

                throw error;
              } finally {
                if (callbackKey) {
                  inFlightUiCallbacks.delete(callbackKey);
                }
              }
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
        const errorMessage =
          error instanceof Error ? error.message : String(error);

        if (isWorkflowCancelledError(errorMessage)) {
          logger.info('Ignoring stale UI action result', {
            type: result.type,
            error: errorMessage,
          });
          return {
            status: 'ignored',
            message: errorMessage,
          };
        }

        logger.error('Failed to handle UI action', {
          type: result.type,
          error: errorMessage,
        });
        return {
          status: 'error',
          message: errorMessage,
        };
      }
    },
    [
      session?.id,
      messageSessionId,
      submit,
      openExternalUrl,
      contentRef,
      isDark,
      t,
    ],
  );
}
