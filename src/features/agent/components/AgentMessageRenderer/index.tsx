import React, { useMemo, useRef, useEffect, memo } from 'react';
import { AgentToolCallGroup } from '../AgentToolCallGroup';
import { ThinkingBubble } from '../shared';
import type {
  MCPContent,
  MCPToolCallContent,
  MCPThinkingContent,
} from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  basicComponentLibrary,
  UIResourceRenderer,
  remoteButtonDefinition,
  remoteTextDefinition,
  remoteCardDefinition,
  remoteImageDefinition,
  remoteStackDefinition,
} from '@mcp-ui/client';

import { AgentMessageRendererProps } from './types';
import { useIsDarkMode } from './hooks/useIsDarkMode';
import { useUIActionHandler } from './hooks/useUIActionHandler';
import { groupContent } from './utils/contentGrouping';
import { CodeBlock } from './components/CodeBlock';
import { MarkdownText } from './components/MarkdownText';
import { STATIC_MARKDOWN_COMPONENTS } from './config/markdown';

const logger = getLogger('AgentMessageRenderer');

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
const AgentMessageRendererImpl: React.FC<AgentMessageRendererProps> = ({
  content,
  message,
  className = '',
  expandResources = false,
  toolResultsMap,
}) => {
  const { openExternalUrl } = useRustBackend();
  const isDark = useIsDarkMode();

  // Memoize markdown components to include dynamic isDark prop
  // This avoids window.matchMedia calls in every code block render
  const markdownComponents = useMemo(
    () => ({
      ...STATIC_MARKDOWN_COMPONENTS,
      code: ({
        children,
        className,
        node,
        ...props
      }: React.ComponentPropsWithoutRef<'code'> & {
        inline?: boolean;
        node?: unknown;
      }) => {
        void node;
        return (
          <CodeBlock isDark={isDark} className={className} {...props}>
            {children}
          </CodeBlock>
        );
      },
    }),
    [isDark],
  );

  // content 결정: message가 있으면 message.content 사용, 없으면 props.content 사용
  // V2 Fix: Prioritize explicit 'content' prop if provided (e.g. for grouped tool calls)
  const finalContent: MCPContent[] = content || message?.content || [];

  // Group consecutive tool calls into blocks for display
  const renderItems = useMemo(
    () => groupContent(finalContent, message),
    [finalContent, message?.thinking, message?.thinkingTime],
  );

  // Keep latest content in a ref to avoid recreating callbacks on each render
  const contentRef = useRef<MCPContent[]>(finalContent);
  useEffect(() => {
    contentRef.current = finalContent;
  }, [finalContent]);

  // Extract UI Action logic to custom hook
  const handleUIAction = useUIActionHandler(contentRef);

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
  }, [expandResources, renderItems]);

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

  const mutableSupportedContentTypes = useMemo(
    () => [...supportedContentTypes],
    [supportedContentTypes],
  );

  const htmlProps = useMemo(
    () => ({
      style: { height: 'auto', maxHeight: 'unset' },
      iframeProps: {
        className: 'h-auto min-h-96 max-h-none',
      },
    }),
    [],
  );

  const handleLinkClick = async (e: React.MouseEvent, url: string) => {
    e.preventDefault();
    try {
      await openExternalUrl(url);
    } catch {
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  if (!renderItems.length) {
    return null;
  }

  return (
    <div className={`flex flex-col gap-2 min-w-0 max-w-full ${className}`}>
      {renderItems.map((item, index) => {
        const key = `${message?.id}_${index}`;

        // Handle specialized tool groups
        if ('type' in item && item.type === 'tool_group_block') {
          const groupBlock = item as {
            type: 'tool_group_block';
            items: MCPToolCallContent[];
          };
          const toolGroupCalls = groupBlock.items.map((tc) => ({
            id: tc.id,
            type: 'function' as const,
            function: { name: tc.name, arguments: tc.arguments },
          }));
          const toolGroupResults = toolGroupCalls.map((call) =>
            toolResultsMap?.get(call.id),
          );

          return (
            <div key={key} className="my-2">
              <AgentToolCallGroup
                message={
                  message ||
                  ({
                    id: 'dummy',
                    role: 'assistant',
                    content: [],
                  } as unknown as Message)
                }
                toolGroup={{ calls: toolGroupCalls }}
                toolResults={toolGroupResults}
                isLast={index === renderItems.length - 1}
                visibleCount={999}
              />
            </div>
          );
        }

        // Handle MCP Content
        const contentItem = item as MCPContent;
        const itemKey = `${message?.id}_${contentItem.type}_${index}`;

        switch (contentItem.type) {
          case 'thinking': {
            const thinkingItem = contentItem as MCPThinkingContent;
            return (
              <div key={itemKey} className="mb-2">
                <ThinkingBubble
                  thinking={thinkingItem.thinking}
                  thinkingTime={thinkingItem.thinkingTime}
                  isStreaming={message?.isStreaming}
                />
              </div>
            );
          }
          case 'text': {
            const textItem = contentItem as { text: string };
            return (
              <MarkdownText
                key={itemKey}
                content={textItem.text}
                components={markdownComponents}
              />
            );
          }
          case 'resource': {
            const resourceItem = contentItem as {
              type: 'resource';
              resource: {
                uri: string;
                mimeType: string;
                text?: string;
                blob?: string;
                _meta?: Record<string, unknown>;
              };
            };

            if (!resourceItem.resource) {
              logger.warn('Resource content is missing resource property', {
                item,
              });
              return null;
            }

            return (
              <div
                key={itemKey}
                ref={(el) => {
                  resourceRefs.current[itemKey] = el;
                }}
                className={
                  expandResources ? 'w-full overflow-visible min-h-96' : ''
                }
              >
                <UIResourceRenderer
                  remoteDomProps={remoteDomProps}
                  onUIAction={handleUIAction}
                  supportedContentTypes={mutableSupportedContentTypes}
                  htmlProps={htmlProps}
                  resource={resourceItem.resource}
                />
              </div>
            );
          }
          case 'image': {
            const imageItem = contentItem as {
              data?: string;
              source?: { data?: string; uri?: string };
              mimeType?: string;
            };
            const imageSrc =
              imageItem.data || imageItem.source?.data || imageItem.source?.uri;
            return imageSrc ? (
              <img
                key={itemKey}
                src={imageSrc}
                alt="Tool output"
                className="max-w-full h-auto rounded-lg shadow-sm"
              />
            ) : null;
          }
          case 'audio': {
            const audioItem = contentItem as {
              data?: string;
              mimeType?: string;
            };
            return audioItem.data ? (
              <audio key={itemKey} controls className="w-full">
                <source src={audioItem.data} type={audioItem.mimeType} />
                Your browser does not support the audio element.
              </audio>
            ) : null;
          }
          case 'resource_link': {
            const linkItem = contentItem as {
              uri: string;
              name: string;
              description?: string;
            };
            return (
              <div key={itemKey} className="p-2 border rounded-lg bg-muted">
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
              <div key={itemKey} className="text-muted-foreground italic">
                [
                {'type' in contentItem
                  ? (contentItem as { type: string }).type
                  : 'unknown'}
                ]
              </div>
            );
        }
      })}
    </div>
  );
};

export const AgentMessageRenderer = memo(AgentMessageRendererImpl);
export default AgentMessageRenderer;
