import React, { useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import { Copy, Check } from 'lucide-react';
import type { MCPContent } from '@/lib/mcp-types';
import type { Message } from '@/models/chat';
import { extractServiceInfoFromContent } from '@/lib/mcp-types';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useClipboard } from '@/hooks/useClipboard';
import { getLogger } from '@/lib/logger';
import {
  basicComponentLibrary,
  UIResourceRenderer,
  UIActionResult,
  remoteButtonDefinition,
  remoteTextDefinition,
  remoteCardDefinition,
  remoteImageDefinition,
  remoteStackDefinition,
} from '@mcp-ui/client';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { useChatContext } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { stringToMCPContentArray } from '@/lib/utils';
import { useAssistantContext } from '@/context/AssistantContext';

const logger = getLogger('MessageRenderer');

interface MessageRendererProps {
  content?: MCPContent[];
  message?: Message;
  className?: string;
}

export const MessageRenderer: React.FC<MessageRendererProps> = ({
  content,
  message,
  className = '',
}) => {
  const { copied, copyToClipboard } = useClipboard();
  const { openExternalUrl } = useRustBackend();
  const { executeToolCall } = useUnifiedMCP();
  const { submit } = useChatContext();
  const { getCurrentSession } = useSessionContext();
  const { getCurrent } = useAssistantContext();
  const tauriCommands = useRustBackend();

  // content 결정: message가 있으면 message.content 사용, 없으면 props.content 사용
  const finalContent: MCPContent[] = message?.content || content || [];

  const handleLinkClick = async (e: React.MouseEvent, url: string) => {
    e.preventDefault();
    logger.info('Opening external URL', { url });

    try {
      await openExternalUrl(url);
      logger.info('External URL opened successfully', { url });
    } catch (error) {
      logger.error(
        'Failed to open external URL via Tauri, falling back to window.open',
        { url, error },
      );
      // Fallback for browser environment
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  const handleUIAction = useCallback(
    async (result: UIActionResult) => {
      logger.info('UI action received', {
        type: result.type,
        payload: result.payload,
      });

      const sessionId = getCurrentSession()?.id;
      const assistantId = getCurrent()?.id;

      if (!sessionId) {
        logger.warn('No active session available for UI action');
        return;
      }

      try {
        switch (result.type) {
          case 'tool': {
            logger.info('Tool call requested from UI', {
              toolName: result.payload.toolName,
              params: result.payload.params,
            });
            const { toolName, params = {} } = result.payload;

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
                logger.info('Executing Tauri command', {
                  strippedCommand,
                  params,
                });

                // Tauri 명령어도 완전한 tool chain으로 처리
                const toolCallId = createId();

                try {
                  let result: string;

                  // 각 Tauri 명령어별로 명시적 처리
                  switch (strippedCommand) {
                    case 'downloadWorkspaceFile': {
                      result = await tauriCommands.downloadWorkspaceFile(
                        params.filePath as string,
                      );
                      break;
                    }
                    case 'exportAndDownloadZip': {
                      result = await tauriCommands.exportAndDownloadZip(
                        params.files as string[],
                        params.packageName as string,
                      );
                      break;
                    }
                    case 'openExternalUrl': {
                      await tauriCommands.openExternalUrl(params.url as string);
                      result = 'External URL opened successfully';
                      break;
                    }
                    default: {
                      throw new Error(
                        `Unsupported Tauri command: ${strippedCommand}`,
                      );
                    }
                  }

                  logger.info('Tauri command completed successfully', {
                    strippedCommand,
                    result,
                  });

                  // 실행 완료 후 tool call + tool result 메시지 쌍을 함께 추가
                  const toolCallMessage: Message = {
                    id: createId(),
                    content: [],
                    role: 'assistant',
                    tool_calls: [
                      {
                        id: toolCallId,
                        type: 'function',
                        function: {
                          name: `tauri:${strippedCommand}`,
                          arguments: JSON.stringify(params),
                        },
                      },
                    ],
                    sessionId,
                    assistantId,
                  };

                  const successMessage: Message = {
                    id: createId(),
                    content: stringToMCPContentArray(`✅ ${result}`),
                    role: 'tool',
                    tool_call_id: toolCallId,
                    sessionId,
                    assistantId,
                  };

                  // 메시지 쌍을 함께 추가
                  await submit([toolCallMessage, successMessage]);
                } catch (error) {
                  logger.error('Tauri command failed', {
                    strippedCommand,
                    error,
                  });

                  // 에러 시에도 tool call + tool result 메시지 쌍을 함께 추가
                  const toolCallMessage: Message = {
                    id: createId(),
                    content: [],
                    role: 'assistant',
                    tool_calls: [
                      {
                        id: toolCallId,
                        type: 'function',
                        function: {
                          name: `tauri:${strippedCommand}`,
                          arguments: JSON.stringify(params),
                        },
                      },
                    ],
                    sessionId,
                    assistantId,
                  };

                  const errorMessage: Message = {
                    id: createId(),
                    content: stringToMCPContentArray(
                      `❌ ${strippedCommand} failed: ${error instanceof Error ? error.message : String(error)}`,
                    ),
                    role: 'tool',
                    tool_call_id: toolCallId,
                    sessionId,
                    assistantId,
                  };

                  // 에러 메시지 쌍을 함께 추가
                  await submit([toolCallMessage, errorMessage]);
                }
              } else {
                logger.warn('Tauri command not found', {
                  strippedCommand,
                  availableMethods: Object.keys(tauriCommands),
                });
              }
            } else {
              // MCP 도구 호출 개선: content에서 service info 추출
              const serviceInfo = extractServiceInfoFromContent(finalContent);
              let finalToolName = toolName;
              if (serviceInfo) {
                const isBaseName =
                  !toolName.includes('__') && !toolName.startsWith('builtin.');

                if (isBaseName) {
                  finalToolName =
                    serviceInfo.backendType === 'ExternalMCP'
                      ? `${serviceInfo.serverName}__${toolName}`
                      : `builtin.${serviceInfo.serverName}__${toolName}`;

                  logger.info('Tool name resolved using service context', {
                    originalName: toolName,
                    resolvedName: finalToolName,
                    serviceInfo,
                  });
                }
              } else {
                logger.warn(
                  'No service context available, using original tool name',
                  {
                    toolName,
                  },
                );
              }

              // 통합된 MCP 도구 호출
              const toolCallId = createId();

              // 실제 도구 실행
              const response = await executeToolCall({
                id: toolCallId,
                type: 'function',
                function: {
                  name: finalToolName,
                  arguments: JSON.stringify(params),
                },
              });

              // 도구 실행 완료 후 tool call + tool result 메시지 쌍을 함께 추가
              const toolCallMessage: Message = {
                id: createId(),
                content: [], // Tool call은 content가 비어있을 수 있음
                role: 'assistant',
                tool_calls: [
                  {
                    id: toolCallId,
                    type: 'function',
                    function: {
                      name: finalToolName,
                      arguments: JSON.stringify(params),
                    },
                  },
                ],
                sessionId,
                assistantId,
              };

              if (response && response.result && response.result.content) {
                const toolResultMessage: Message = {
                  id: createId(),
                  content: response.result.content,
                  role: 'tool',
                  tool_call_id: toolCallId,
                  sessionId,
                  assistantId,
                };
                // 메시지 쌍을 함께 추가
                await submit([toolCallMessage, toolResultMessage]);
              } else if (response && response.error) {
                const errorResultMessage: Message = {
                  id: createId(),
                  content: stringToMCPContentArray(
                    `❌ Tool execution failed: ${response.error.message}`,
                  ),
                  role: 'tool',
                  tool_call_id: toolCallId,
                  sessionId,
                  assistantId,
                };
                // 에러 시에도 메시지 쌍을 함께 추가
                await submit([toolCallMessage, errorResultMessage]);
              }
            }
            break;
          }

          case 'intent': {
            logger.info('Intent requested from UI', {
              intent: result.payload.intent,
              params: result.payload.params,
            });

            // Intent를 자연어 프롬프트로 변환
            const intentText = `User intent: ${result.payload.intent}`;
            const paramsText = result.payload.params
              ? `\nParameters: ${JSON.stringify(result.payload.params, null, 2)}`
              : '';

            const intentMessage: Message = {
              id: createId(),
              content: stringToMCPContentArray(intentText + paramsText),
              role: 'user',
              sessionId,
              assistantId,
            };

            await submit([intentMessage]);
            break;
          }

          case 'prompt': {
            logger.info('User prompt requested from UI', {
              prompt: result.payload.prompt,
            });

            const promptMessage: Message = {
              id: createId(),
              content: stringToMCPContentArray(result.payload.prompt),
              role: 'user',
              sessionId,
              assistantId,
            };

            await submit([promptMessage]);
            break;
          }

          case 'link': {
            logger.info('External link requested from UI', {
              url: result.payload.url,
            });

            await openExternalUrl(result.payload.url);
            break;
          }

          case 'notify': {
            logger.info('Notification requested from UI', {
              message: result.payload.message,
            });

            // 알림을 시스템 메시지로 채팅에 추가
            const notificationMessage: Message = {
              id: createId(),
              content: stringToMCPContentArray(`🔔 ${result.payload.message}`),
              role: 'system',
              sessionId,
              assistantId,
            };

            await submit([notificationMessage]);
            break;
          }

          default: {
            logger.warn('Unknown UI action type', {
              type: (result as { type: string }).type,
              result,
            });
            break;
          }
        }
      } catch (error) {
        logger.error('Failed to handle UI action', {
          type: result.type,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    },
    [executeToolCall, submit, getCurrentSession, getCurrent, finalContent],
  );

  if (!finalContent.length) {
    return null;
  }

  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {finalContent.map((item, index) => {
        switch (item.type) {
          case 'text': {
            const textItem = item as { text: string };

            return (
              <div
                key={index}
                className="group relative text-sm leading-relaxed"
              >
                {/* Copy button for individual text */}
                <button
                  onClick={async () => {
                    try {
                      await copyToClipboard(textItem.text);
                    } catch (err) {
                      logger.error('Failed to copy text content', err);
                    }
                  }}
                  className="absolute top-2 right-2 flex items-center gap-1 px-2 py-1 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-all opacity-0 group-hover:opacity-100 z-10"
                  aria-label="Copy text content"
                >
                  {copied ? <Check size={12} /> : <Copy size={12} />}
                  {copied ? 'Copied!' : 'Copy'}
                </button>

                <ReactMarkdown
                  skipHtml={false}
                  remarkPlugins={[]}
                  rehypePlugins={[]}
                  components={{
                    // ReactMarkdown은 기본적으로 관대하므로 간단한 fallback만 제공
                    p: ({ children, ...props }) => <p {...props}>{children}</p>,
                    code: ({ children, className }) => (
                      <code className={className}>{children}</code>
                    ),
                    pre: ({ children, ...props }) => (
                      <pre {...props}>{children}</pre>
                    ),
                  }}
                >
                  {textItem.text}
                </ReactMarkdown>
              </div>
            );
          }
          case 'resource':
            logger.info('resource : ', { resource: item.resource });
            return (
              <UIResourceRenderer
                key={index}
                remoteDomProps={{
                  library: basicComponentLibrary,
                  remoteElements: [
                    remoteButtonDefinition,
                    remoteTextDefinition,
                    remoteCardDefinition,
                    remoteImageDefinition,
                    remoteStackDefinition,
                  ],
                }}
                onUIAction={handleUIAction}
                supportedContentTypes={['rawHtml', 'externalUrl', 'remoteDom']}
                resource={item.resource}
              />
            );
          case 'image': {
            const imageItem = item as {
              data?: string;
              source?: { data?: string; uri?: string };
              mimeType?: string;
            };
            const imageSrc =
              imageItem.data || imageItem.source?.data || imageItem.source?.uri;
            return imageSrc ? (
              <img
                key={index}
                src={imageSrc}
                alt="Tool output"
                className="max-w-full h-auto rounded-lg shadow-sm"
              />
            ) : null;
          }
          case 'audio': {
            const audioItem = item as { data?: string; mimeType?: string };
            return audioItem.data ? (
              <audio key={index} controls className="w-full">
                <source src={audioItem.data} type={audioItem.mimeType} />
                Your browser does not support the audio element.
              </audio>
            ) : null;
          }
          case 'resource_link': {
            const linkItem = item as {
              uri: string;
              name: string;
              description?: string;
            };
            return (
              <div key={index} className="p-2 border rounded-lg bg-gray-50">
                <a
                  href={linkItem.uri}
                  onClick={(e) => handleLinkClick(e, linkItem.uri)}
                  className="text-blue-600 hover:text-blue-800 underline"
                >
                  {linkItem.name}
                </a>
                {linkItem.description && (
                  <div className="text-sm text-gray-600 mt-1">
                    {linkItem.description}
                  </div>
                )}
              </div>
            );
          }
          default:
            return (
              <div key={index} className="text-gray-500 italic">
                [{'type' in item ? (item as { type: string }).type : 'unknown'}]
              </div>
            );
        }
      })}
    </div>
  );
};

export default MessageRenderer;
