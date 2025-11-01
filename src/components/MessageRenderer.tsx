import React, { useCallback, useMemo, useRef, useEffect } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Copy, Check } from 'lucide-react';
import type { MCPContent } from '@/lib/mcp-types';
import type { Message } from '@/models/chat';
import { extractServiceInfoFromContent } from '@/lib/mcp-types';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useClipboard } from '@/hooks/useClipboard';
import { getLogger } from '@/lib/logger';
import { Highlight, themes } from 'prism-react-renderer';
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
import { useChatActions } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { stringToMCPContentArray } from '@/lib/utils';
import { useAssistantContext } from '@/context/AssistantContext';
import {
  createSystemMessage,
  createUserMessage,
  createToolMessagePair,
} from '@/lib/chat-utils';

const logger = getLogger('MessageRenderer');

interface MessageRendererProps {
  content?: MCPContent[];
  message?: Message;
  className?: string;
  /** Allow resource blocks to expand to their content height (no internal scroll) */
  expandResources?: boolean;
}

export const MessageRenderer: React.FC<MessageRendererProps> = ({
  content,
  message,
  className = '',
  expandResources = false,
}) => {
  const { copied, copyToClipboard } = useClipboard();
  const { openExternalUrl } = useRustBackend();
  const { executeToolCall } = useUnifiedMCP();
  const { submit } = useChatActions();
  const { getCurrentSession } = useSessionContext();
  const { getCurrent } = useAssistantContext();
  const tauriCommands = useRustBackend();

  // content 결정: message가 있으면 message.content 사용, 없으면 props.content 사용
  const finalContent: MCPContent[] = message?.content || content || [];

  // Keep latest content in a ref to avoid recreating callbacks on each render
  const contentRef = useRef<MCPContent[]>(finalContent);
  useEffect(() => {
    contentRef.current = finalContent;
  }, [finalContent]);

  // Refs to resource wrappers so we can observe size changes and scroll into view
  const resourceRefs = useRef<Record<string, HTMLDivElement | null>>({});

  // When resources are allowed to expand, watch size changes and scroll them into view
  useEffect(() => {
    if (!expandResources) return;

    const observers: ResizeObserver[] = [];
    Object.values(resourceRefs.current).forEach((el) => {
      if (!el) return;
      let lastHeight = el.getBoundingClientRect().height;
      const ro = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const height = entry.contentRect.height;
          if (height > lastHeight) {
            // Ensure the newly expanded content is visible in the scrollable container
            try {
              el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            } catch {
              // ignore
            }
          }
          lastHeight = height;
        }
      });
      ro.observe(el);
      observers.push(ro);
    });

    return () => observers.forEach((o) => o.disconnect());
  }, [expandResources, finalContent]);

  // Memoize renderer props to keep identity stable across re-renders
  const remoteDomProps = useMemo(
    () => ({
      library: basicComponentLibrary,
      remoteElements: [
        remoteButtonDefinition,
        remoteTextDefinition,
        remoteCardDefinition,
        remoteImageDefinition,
        remoteStackDefinition,
      ],
    }),
    [],
  );

  const supportedContentTypes = useMemo(
    () => ['rawHtml', 'externalUrl', 'remoteDom'] as const,
    [],
  );

  const handleLinkClick = async (e: React.MouseEvent, url: string) => {
    e.preventDefault();

    try {
      await openExternalUrl(url);
    } catch {
      // Fallback for browser environment
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  const handleUIAction = useCallback(
    async (result: UIActionResult) => {
      const sessionId = getCurrentSession()?.id;
      const assistantId = getCurrent()?.id;

      if (!sessionId) {
        return;
      }

      try {
        switch (result.type) {
          case 'tool': {
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

                  // 실행 완료 후 tool call + tool result 메시지 쌍을 함께 추가
                  const [toolCallMessage, successMessage] =
                    createToolMessagePair(
                      `tauri:${strippedCommand}`,
                      params,
                      stringToMCPContentArray(`✅ ${result}`),
                      toolCallId,
                      sessionId,
                      assistantId,
                      'ui',
                    );

                  // 메시지 쌍을 함께 추가
                  await submit([toolCallMessage, successMessage]);
                } catch (error) {
                  logger.error('Tauri command failed', {
                    strippedCommand,
                    error,
                  });

                  // 에러 시에도 tool call + tool result 메시지 쌍을 함께 추가
                  const [toolCallMessage, errorMessage] = createToolMessagePair(
                    `tauri:${strippedCommand}`,
                    params,
                    stringToMCPContentArray(
                      `❌ ${strippedCommand} failed: ${error instanceof Error ? error.message : String(error)}`,
                    ),
                    toolCallId,
                    sessionId,
                    assistantId,
                    'ui',
                  );

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
              // MCP 도구 호출 개선: latest content에서 service info 추출
              const serviceInfo = extractServiceInfoFromContent(
                contentRef.current,
              );
              let finalToolName = toolName;
              if (serviceInfo) {
                const isBaseName =
                  !toolName.includes('__') && !toolName.startsWith('builtin.');

                if (isBaseName) {
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
              if (response && response.result && response.result.content) {
                const [toolCallMessage, toolResultMessage] =
                  createToolMessagePair(
                    finalToolName,
                    params,
                    response.result.content,
                    toolCallId,
                    sessionId,
                    assistantId,
                    'ui',
                  );
                // 메시지 쌍을 함께 추가
                await submit([toolCallMessage, toolResultMessage]);
              } else if (response && response.error) {
                const [toolCallMessage, errorResultMessage] =
                  createToolMessagePair(
                    finalToolName,
                    params,
                    stringToMCPContentArray(
                      `❌ Tool execution failed: ${response.error.message}`,
                    ),
                    toolCallId,
                    sessionId,
                    assistantId,
                    'ui',
                  );
                // 에러 시에도 메시지 쌍을 함께 추가
                await submit([toolCallMessage, errorResultMessage]);
              }
            }
            return { status: 'tool-submitted', tool: toolName };
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
              assistantId,
              'ui',
            );

            await submit([intentMessage]);
            return {
              status: 'intent-submitted',
              intent: result.payload.intent,
            };
          }

          case 'prompt': {
            const promptMessage = createUserMessage(
              result.payload.prompt,
              sessionId,
              assistantId,
              'ui',
            );

            await submit([promptMessage]);
            return { status: 'prompt-submitted' };
          }

          case 'link': {
            await openExternalUrl(result.payload.url);
            return { status: 'link-opened' };
          }

          case 'notify': {
            // 알림을 시스템 메시지로 채팅에 추가
            const notificationMessage = createSystemMessage(
              `🔔 ${result.payload.message}`,
              sessionId,
              assistantId,
              'ui',
            );

            await submit([notificationMessage]);
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
    [executeToolCall, submit, getCurrentSession, getCurrent],
  );

  if (!finalContent.length) {
    return null;
  }

  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {finalContent.map((item, index) => {
        const key = `${message?.id}_${item.type}_${index}`;
        switch (item.type) {
          case 'text': {
            const textItem = item as { text: string };

            return (
              <div
                key={key}
                className="group relative text-sm leading-relaxed overflow-x-hidden break-words"
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
                  remarkPlugins={[remarkGfm]}
                  rehypePlugins={[]}
                  components={{
                    p: ({ children, ...props }) => (
                      <p className="mb-2 last:mb-0" {...props}>
                        {children}
                      </p>
                    ),
                    code: ({ children, className, ...props }) => {
                      // Distinguish inline code vs block code
                      // ReactMarkdown passes className="language-xxx" for code blocks
                      const match = /language-(\w+)/.exec(className || '');
                      const language = match ? match[1] : '';

                      if (!language) {
                        // Inline code
                        return (
                          <code
                            className="px-1.5 py-0.5 bg-muted rounded text-sm font-mono border border-border"
                            {...props}
                          >
                            {children}
                          </code>
                        );
                      }

                      // Block code with syntax highlighting
                      const code = String(children).replace(/\n$/, '');

                      // Detect dark mode
                      const isDark =
                        typeof window !== 'undefined' &&
                        window.matchMedia('(prefers-color-scheme: dark)')
                          .matches;

                      return (
                        <Highlight
                          theme={isDark ? themes.oneDark : themes.oneLight}
                          code={code}
                          language={language}
                        >
                          {({
                            className: highlightClassName,
                            style,
                            tokens,
                            getLineProps,
                            getTokenProps,
                          }) => (
                            <code
                              className={`${highlightClassName} block font-mono text-sm`}
                              style={style}
                            >
                              {tokens.map((line, i) => (
                                <div key={i} {...getLineProps({ line })}>
                                  {line.map((token, key) => (
                                    <span
                                      key={key}
                                      {...getTokenProps({ token })}
                                    />
                                  ))}
                                </div>
                              ))}
                            </code>
                          )}
                        </Highlight>
                      );
                    },
                    pre: ({ children, ...props }) => (
                      <pre
                        className="overflow-x-auto bg-muted rounded-lg p-4 my-3 border border-border max-w-full"
                        {...props}
                      >
                        {children}
                      </pre>
                    ),
                    h1: ({ children, ...props }) => (
                      <h1 className="text-2xl font-bold mb-3 mt-4" {...props}>
                        {children}
                      </h1>
                    ),
                    h2: ({ children, ...props }) => (
                      <h2 className="text-xl font-bold mb-2 mt-3" {...props}>
                        {children}
                      </h2>
                    ),
                    h3: ({ children, ...props }) => (
                      <h3
                        className="text-lg font-semibold mb-2 mt-2"
                        {...props}
                      >
                        {children}
                      </h3>
                    ),
                    ul: ({ children, ...props }) => (
                      <ul
                        className="list-disc list-inside mb-2 space-y-1"
                        {...props}
                      >
                        {children}
                      </ul>
                    ),
                    ol: ({ children, ...props }) => (
                      <ol
                        className="list-decimal list-inside mb-2 space-y-1"
                        {...props}
                      >
                        {children}
                      </ol>
                    ),
                    li: ({ children, ...props }) => (
                      <li className="ml-2" {...props}>
                        {children}
                      </li>
                    ),
                    blockquote: ({ children, ...props }) => (
                      <blockquote
                        className="border-l-4 border-primary pl-4 italic my-2 text-muted-foreground"
                        {...props}
                      >
                        {children}
                      </blockquote>
                    ),
                    a: ({ children, href, ...props }) => (
                      <a
                        href={href}
                        className="text-primary hover:underline"
                        target="_blank"
                        rel="noopener noreferrer"
                        {...props}
                      >
                        {children}
                      </a>
                    ),
                  }}
                >
                  {textItem.text}
                </ReactMarkdown>
              </div>
            );
          }
          case 'resource':
            // Prefer a stable, unique key to ensure proper mount/unmount semantics
            // Use message.id + resource.uri to avoid index-based reordering issues
            // Also, pass stable props to avoid unnecessary teardown in the renderer
            return (
              <div
                key={key}
                ref={(el) => {
                  resourceRefs.current[key] = el;
                }}
                className={
                  expandResources ? 'w-full overflow-visible min-h-[50vh]' : ''
                }
              >
                <UIResourceRenderer
                  remoteDomProps={remoteDomProps}
                  onUIAction={handleUIAction}
                  supportedContentTypes={[...supportedContentTypes]}
                  htmlProps={{
                    autoResizeIframe: { height: true },
                    style: { height: 'auto', maxHeight: 'unset' },
                    iframeProps: {
                      className: 'h-auto min-h-[50vh] max-h-none',
                      onLoad: (ev: React.SyntheticEvent<HTMLIFrameElement>) => {
                        try {
                          const el =
                            ev?.currentTarget as HTMLIFrameElement | null;
                          if (!el) return;
                          const doc =
                            el.contentDocument || el.contentWindow?.document;
                          const height =
                            doc?.documentElement?.scrollHeight ||
                            doc?.body?.scrollHeight;
                          if (height) {
                            el.style.height = `${height}px`;
                          }
                        } catch {
                          // ignore cross-origin or other errors
                        }
                      },
                    },
                  }}
                  resource={item.resource}
                />
              </div>
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
                key={key}
                src={imageSrc}
                alt="Tool output"
                className="max-w-full h-auto rounded-lg shadow-sm"
              />
            ) : null;
          }
          case 'audio': {
            const audioItem = item as { data?: string; mimeType?: string };
            return audioItem.data ? (
              <audio key={key} controls className="w-full">
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
              <div key={key} className="p-2 border rounded-lg bg-muted">
                <a
                  href={linkItem.uri}
                  onClick={(e) => handleLinkClick(e, linkItem.uri)}
                  className="text-primary hover:text-primary/90 underline"
                >
                  {linkItem.name}
                </a>
                {linkItem.description && (
                  <div className="text-sm text-muted-foreground mt-1">
                    {linkItem.description}
                  </div>
                )}
              </div>
            );
          }
          default:
            return (
              <div key={key} className="text-muted-foreground italic">
                [{'type' in item ? (item as { type: string }).type : 'unknown'}]
              </div>
            );
        }
      })}
    </div>
  );
};

export default MessageRenderer;
