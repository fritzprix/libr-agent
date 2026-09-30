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
} from '@/features/agent/lib/markdown-document-export';
import { useTranslation } from 'react-i18next';

const logger = getLogger('AgentMessageRenderer');

function readStringParam(
  params: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = params[key];
  return typeof value === 'string' ? value : undefined;
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
                labels: {
                  markdownSuccess: t(
                    'agent.bubble.actionBar.exportMarkdownSuccess',
                  ),
                  pdfSuccess: t('agent.bubble.actionBar.exportPdfSuccess'),
                  openFile: t('agent.bubble.actionBar.exportOpenFile'),
                  openFileError: t(
                    'agent.bubble.actionBar.exportOpenFileError',
                  ),
                  cancelled: t('agent.bubble.actionBar.exportCancelled'),
                  markdownError: t('agent.bubble.actionBar.exportError'),
                  pdfError: t('agent.bubble.actionBar.exportPdfError'),
                },
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
