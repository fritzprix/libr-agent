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
import { useTheme } from 'next-themes';
import { useUIActionHandler } from './hooks/useUIActionHandler';
import type { AgentMessageRendererProps, RenderItem } from './types';
import { groupContent } from './utils/contentGrouping';
import {
  releaseDisplayMediaCacheSession,
  retainDisplayMediaCacheSession,
} from './utils/displayMediaCache';
import {
  buildUiResourceThemeStyleTag,
  getUiResourceThemeVars,
} from './utils/injectUiResourceTheme';
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
  followChatScroll = true,
}) => {
  const { openExternalUrl } = useRustBackend();
  const { resolvedTheme } = useTheme();
  const isDark = useIsDarkMode();
  // next-themes leaves resolvedTheme undefined until mounted; do not inject a
  // speculative light theme (defaultTheme is dark, so undefined !== dark).
  const themeReady =
    resolvedTheme === 'dark' || resolvedTheme === 'light';

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

  // Theme tokens come from static maps keyed by resolvedTheme — not
  // getComputedStyle — so a dark session load cannot bake light :root values
  // into the iframe before `<html class="dark">` is applied.
  const themeCssVars = useMemo(
    () => (themeReady ? getUiResourceThemeVars(isDark) : null),
    [themeReady, isDark],
  );
  const themeStyleTag = useMemo(
    () =>
      themeCssVars
        ? buildUiResourceThemeStyleTag(isDark, themeCssVars)
        : null,
    [isDark, themeCssVars],
  );

  // Expanded tool results: let autoResizeIframe grow with content (no 384px /
  // max-h-96-style cap — that caused an inner iframe scrollbar). Compact /
  // inline resources keep a fixed viewport with an 80vh ceiling.
  const htmlProps = useMemo(() => {
    const backgroundColor = themeCssVars?.['--background'];

    return {
      autoResizeIframe: { height: true, width: false },
      style: expandResources
        ? { width: '100%', minHeight: '200px' }
        : { height: '384px', maxHeight: '80vh' },
      iframeProps: {
        className: 'w-full',
        style: {
          colorScheme: isDark ? 'dark' : 'light',
          ...(backgroundColor ? { backgroundColor } : {}),
        },
      },
    };
  }, [expandResources, isDark, themeCssVars]);

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
              followChatScroll={followChatScroll}
              resourceRefs={resourceRefs}
              markdownComponents={markdownComponents}
              remoteDomProps={remoteDomProps}
              supportedContentTypes={mutableSupportedContentTypes}
              htmlProps={htmlProps}
              themeStyleTag={themeStyleTag}
              themeKey={resolvedTheme ?? 'theme-pending'}
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
