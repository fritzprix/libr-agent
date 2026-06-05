import React, { memo, useEffect, useMemo, useRef } from 'react';
import {
  basicComponentLibrary,
  remoteButtonDefinition,
  remoteCardDefinition,
  remoteImageDefinition,
  remoteStackDefinition,
  remoteTextDefinition,
} from '@mcp-ui/client';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import { CodeBlock } from './components/CodeBlock';
import { ContentItemRenderer } from './components/ContentItemRenderer';
import { STATIC_MARKDOWN_COMPONENTS } from './config/markdown';
import { useIsDarkMode } from '@/hooks/use-is-dark-mode';
import { useUIActionHandler } from './hooks/useUIActionHandler';
import type { AgentMessageRendererProps, RenderItem } from './types';
import { groupContent } from './utils/contentGrouping';
import {
  releaseDisplayMediaCacheSession,
  retainDisplayMediaCacheSession,
} from './utils/displayMediaCache';
import { isSafeExternalUrl } from './utils/url';

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

  const markdownComponents = useMemo(
    () => ({
      ...STATIC_MARKDOWN_COMPONENTS,
      code: ({
        children,
        className,
        node: _node, // ReactMarkdown passes node prop which is invalid on HTML element, destructure to filter it out
        ...props
      }: React.ComponentPropsWithoutRef<'code'> & {
        inline?: boolean;
        node?: unknown;
      }) => {
        void _node;
        return (
          <CodeBlock isDark={isDark} className={className} {...props}>
            {children}
          </CodeBlock>
        );
      },
    }),
    [isDark],
  );

  const finalContent = content || message?.content || [];
  const renderItems = useMemo(
    () => groupContent(finalContent, message),
    [finalContent, message?.thinking, message?.thinkingTime],
  );

  useEffect(() => {
    const sessionId = message?.sessionId;
    if (!sessionId) {
      return;
    }

    retainDisplayMediaCacheSession(sessionId);
    return () => {
      releaseDisplayMediaCacheSession(sessionId);
    };
  }, [message?.sessionId]);

  const contentRef = useRef(finalContent);
  useEffect(() => {
    contentRef.current = finalContent;
  }, [finalContent]);

  const handleUIAction = useUIActionHandler(contentRef);
  const resourceRefs = useRef<Record<string, HTMLDivElement | null>>({});

  useEffect(() => {
    if (!expandResources) {
      return;
    }

    const observers: ResizeObserver[] = [];
    Object.values(resourceRefs.current).forEach((element) => {
      if (!element) {
        return;
      }

      let lastHeight = element.getBoundingClientRect().height;
      const observer = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const height = entry.contentRect.height;
          if (height > lastHeight) {
            try {
              element.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            } catch {
              // ignore
            }
          }
          lastHeight = height;
        }
      });

      observer.observe(element);
      observers.push(observer);
    });

    return () => observers.forEach((observer) => observer.disconnect());
  }, [expandResources, renderItems]);

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
      autoResizeIframe: { height: true, width: false },
      style: { height: '384px', maxHeight: '80vh' },
      iframeProps: {
        className: 'w-full',
      },
    }),
    [],
  );

  const handleLinkClick = async (
    event: React.MouseEvent<HTMLAnchorElement>,
    url: string,
  ) => {
    event.preventDefault();

    if (!isSafeExternalUrl(url)) {
      logger.warn('Blocked attempt to open unsafe URL', { url });
      return;
    }

    try {
      await openExternalUrl(url);
    } catch {
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  const displayItems = useMemo((): RenderItem[] => {
    const hasUIResource = renderItems.some((item) => item.type === 'resource');
    return hasUIResource
      ? renderItems.filter((item) => item.type !== 'text')
      : renderItems;
  }, [renderItems]);

  if (!displayItems.length) {
    return null;
  }

  return (
    <div className={`flex min-w-0 max-w-full flex-col gap-2 ${className}`}>
      {displayItems.map((item, index) => {
        const itemKey =
          item.type === 'tool_group_block'
            ? `${message?.id}_tool-group_${index}`
            : `${message?.id}_${item.type}_${index}`;

        return (
          <div key={itemKey}>
            <ContentItemRenderer
              item={item}
              itemKey={itemKey}
              isLast={index === displayItems.length - 1}
              message={message}
              expandResources={expandResources}
              toolResultsMap={toolResultsMap}
              resourceRefs={resourceRefs}
              markdownComponents={markdownComponents}
              remoteDomProps={remoteDomProps}
              supportedContentTypes={mutableSupportedContentTypes}
              htmlProps={htmlProps}
              onUIAction={handleUIAction}
              onLinkClick={handleLinkClick}
            />
          </div>
        );
      })}
    </div>
  );
};

export const AgentMessageRenderer = memo(AgentMessageRendererImpl);
export default AgentMessageRenderer;
