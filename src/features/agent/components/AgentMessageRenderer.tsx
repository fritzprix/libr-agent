import React, { useCallback, useMemo, useRef, useEffect } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import 'katex/dist/katex.min.css';
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
// import { createId } from '@paralleldrive/cuid2'; // Removed as it's not used directly anymore (handled in backend wrapper)
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  createSystemMessage,
  createUserMessage,
  createToolMessagePair,
} from '@/lib/chat-utils';
import { handleUserToolCall } from '@/lib/backend'; // Import type-safe wrapper
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('AgentMessageRenderer');

interface AgentMessageRendererProps {
  content?: MCPContent[];
  message?: Message;
  className?: string;
  /** Allow resource blocks to expand to their content height (no internal scroll) */
  expandResources?: boolean;
}

/**
 * AgentMessageRenderer - Agent V2용 메시지 렌더러
 *
 * Legacy MessageRenderer와의 주요 차이점:
 * 1. Context 의존성: ChatContext → AgentChatContext, AgentSessionContext
 * 2. Tool execution: createToolMessagePair 제거, Rust가 메시지 생성 담당
 * 3. UI Action: Tool call만 실행, Rust가 자동으로 re-submit 조건 체크
 * 4. Submit: submit([messages]) → submit(message) 단일 메시지
 *
 * Reference: elaborated_idea.md - UI Resource Auto-Pause/Resume Mechanism
 */
export const AgentMessageRenderer: React.FC<AgentMessageRendererProps> = ({
  content,
  message,
  className = '',
  expandResources = false,
}) => {
  const { copied, copyToClipboard } = useClipboard();
  const { openExternalUrl } = useRustBackend(); // Removed callToolUnified
  const { submit, injectMessages } = useAgentChatActions();
  const { session } = useAgentSessionState();
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
              `🔔 ${result.payload.message}`,
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
    [session?.id, submit, openExternalUrl, tauriCommands],
  );

  if (!finalContent.length) {
    return null;
  }

  return (
    // min-w-0 is crucial for flex items to shrink below their content size, preventing overflow
    <div className={`flex flex-col gap-2 min-w-0 max-w-full ${className}`}>
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
                  remarkPlugins={[remarkGfm, remarkMath]}
                  rehypePlugins={[rehypeKatex]}
                  components={{
                    p: ({ children, ...props }) => (
                      <p className="mb-2 last:mb-0" {...props}>
                        {children}
                      </p>
                    ),
                    code: ({
                      children,
                      className,
                      ...props
                    }: React.ComponentPropsWithoutRef<'code'> & {
                      inline?: boolean;
                      node?: unknown;
                    }) => {
                      // Distinguish inline code vs block code
                      // ReactMarkdown passes className="language-xxx" for code blocks
                      const match = /language-(\w+)/.exec(className || '');
                      const language = match ? match[1] : '';

                      if (!language) {
                        // Inline code
                        return (
                          <code
                            className="px-1.5 py-0.5 bg-muted rounded text-sm font-mono border border-border break-all"
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
                    table: ({ children, ...props }) => (
                      <div className="overflow-x-auto w-full max-w-full my-4 border rounded-lg">
                        <table className="w-full text-sm text-left" {...props}>
                          {children}
                        </table>
                      </div>
                    ),
                    thead: ({ children, ...props }) => (
                      <thead
                        className="bg-muted/50 text-muted-foreground"
                        {...props}
                      >
                        {children}
                      </thead>
                    ),
                    tbody: ({ children, ...props }) => (
                      <tbody className="divide-y divide-border" {...props}>
                        {children}
                      </tbody>
                    ),
                    tr: ({ children, ...props }) => (
                      <tr
                        className="border-b border-border last:border-0 hover:bg-muted/30 transition-colors"
                        {...props}
                      >
                        {children}
                      </tr>
                    ),
                    th: ({ children, ...props }) => (
                      <th className="px-4 py-3 font-medium" {...props}>
                        {children}
                      </th>
                    ),
                    td: ({ children, ...props }) => (
                      <td className="px-4 py-3" {...props}>
                        {children}
                      </td>
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
                    ul: ({
                      children,
                      ...props
                    }: React.ComponentPropsWithoutRef<'ul'> & {
                      ordered?: boolean;
                      node?: unknown;
                    }) => (
                      <ul
                        className="list-disc list-inside mb-2 space-y-1"
                        {...props}
                      >
                        {children}
                      </ul>
                    ),
                    ol: ({
                      children,
                      ...props
                    }: React.ComponentPropsWithoutRef<'ol'> & {
                      node?: unknown;
                      ordered?: boolean;
                    }) => (
                      <ol
                        className="list-decimal list-inside mb-2 space-y-1"
                        {...props}
                      >
                        {children}
                      </ol>
                    ),
                    li: ({
                      children,
                      ...props
                    }: React.ComponentPropsWithoutRef<'li'> & {
                      node?: unknown;
                      ordered?: boolean;
                    }) => (
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
                    strong: ({ children, ...props }) => (
                      <strong className="font-bold" {...props}>
                        {children}
                      </strong>
                    ),
                    em: ({ children, ...props }) => (
                      <em className="italic" {...props}>
                        {children}
                      </em>
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
                    style: { height: 'auto', maxHeight: 'unset' },
                    iframeProps: {
                      className: 'h-auto min-h-[50vh] max-h-none',
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

export default AgentMessageRenderer;
