import { useCallback } from 'react';
import { UIActionResult } from '@mcp-ui/client';
import { getLogger } from '@/lib/logger';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import {
  createSystemMessage,
  createUserMessage,
  createToolMessagePair,
} from '@/lib/chat-utils';
import { handleUserToolCall } from '@/lib/backend';
import { extractServiceInfoFromContent, MCPContent } from '@/lib/mcp';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('AgentMessageRenderer:useUIActionHandler');

export function useUIActionHandler(
  contentRef: React.MutableRefObject<MCPContent[]>,
) {
  const { submit, injectMessages } = useAgentChatActions();
  const { session } = useAgentSessionState();
  const tauriCommands = useRustBackend(); // This is the full backend object
  const { openExternalUrl } = tauriCommands;

  /**
   * Handle UI Action from UIResourceRenderer
   *
   * V2 Simplified Logic:
   * - Tool execution만 수행, message pair 생성은 Rust가 담당
   * - UI Resource 감지는 Rust가 수행 (hasToolCall && !hasUIResource)
   * - Frontend는 agent:event로 결과 수신만 함
   */
  const handleUIAction = useCallback(
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

            // prefix 기반 라우팅: tauri: 접두사가 있으면 내부 Tauri 명령어로 처리
            if (toolName.startsWith('tauri:')) {
              const [, strippedCommand] = toolName.split('tauri:');

              // tauriCommands 객체에서 해당 메서드가 존재하는지 확인
              if (
                strippedCommand &&
                typeof tauriCommands[
                  strippedCommand as keyof typeof tauriCommands
                ] === 'function'
              ) {
                try {
                  let resultText: string;

                  // 각 Tauri 명령어별로 명시적 처리
                  switch (strippedCommand) {
                    case 'downloadWorkspaceFile': {
                      resultText = await tauriCommands.downloadWorkspaceFile(
                        params.filePath as string,
                        sessionId,
                      );
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
              // MCP 도구 호출: latest content에서 service info 추출
              const serviceInfo = extractServiceInfoFromContent(
                contentRef.current,
              );

              let finalToolName = toolName;
              if (serviceInfo) {
                const isBaseName =
                  !toolName.includes('__') && !toolName.startsWith('builtin_');

                logger.debug('UI Action Tool Call - Name Resolution', {
                  originalToolName: toolName,
                  isBaseName,
                  backendType: serviceInfo.backendType,
                  serverName: serviceInfo.serverName,
                });

                if (isBaseName) {
                  // Web MCP (BuiltInWeb) & Native (BuiltInRust) 도구는 builtin_ prefix 필요
                  if (
                    serviceInfo.backendType === 'BuiltInWeb' ||
                    serviceInfo.backendType === 'BuiltInRust'
                  ) {
                    finalToolName = `builtin_${serviceInfo.serverName}__${toolName}`;
                  } else {
                    finalToolName = `${serviceInfo.serverName}__${toolName}`;
                  }
                }
              } else {
                logger.warn(
                  'No service context available, using original tool name',
                  {
                    toolName,
                  },
                );
              }

              // 통합된 MCP 도구 호출 (V2: Rust Single Backend)
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
            // Intent를 자연어 프롬프트로 변환
            const intentText = `User intent: ${result.payload.intent}`;
            const paramsText = result.payload.params
              ? `\nParameters: ${JSON.stringify(result.payload.params, null, 2)}`
              : '';

            const intentMessage = createUserMessage(
              intentText + paramsText,
              sessionId,
              undefined, // assistantId는 session에 binding됨
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
              undefined, // assistantId는 session에 binding됨
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
            // 알림을 시스템 메시지로 채팅에 추가
            const notificationMessage = createSystemMessage(
              `[Notification] ${result.payload.message}`,
              sessionId,
              undefined, // assistantId는 session에 binding됨
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
    [session?.id, submit, openExternalUrl, tauriCommands, injectMessages, contentRef],
  );

  return { handleUIAction };
}
