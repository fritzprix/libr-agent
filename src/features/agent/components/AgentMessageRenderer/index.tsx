import React, { useMemo, useRef, useEffect, memo, useState } from 'react';
import { Copy, Check, Download } from 'lucide-react';
import { toast } from 'sonner';
import { AgentToolGroupBlock } from './components/AgentToolGroupBlock';
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
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';

const logger = getLogger('AgentMessageRenderer');
const DISPLAY_MEDIA_CACHE_MAX_BYTES = 64 * 1024 * 1024;
interface DisplayMediaCacheEntry {
  url: string;
  size: number;
}

interface SessionDisplayMediaCache {
  entries: Map<string, DisplayMediaCacheEntry>;
  totalBytes: number;
  mountedRenderers: number;
}

const displayMediaCaches = new Map<string, SessionDisplayMediaCache>();
type ToolGroupBlock = {
  type: 'tool_group_block';
  items: MCPToolCallContent[];
};
type RenderItem = MCPContent | ToolGroupBlock;

function estimateBase64Bytes(value: string): number {
  return Math.floor((value.length * 3) / 4);
}

function getSessionDisplayMediaCache(
  sessionId: string,
): SessionDisplayMediaCache {
  const existing = displayMediaCaches.get(sessionId);
  if (existing) {
    return existing;
  }

  const created: SessionDisplayMediaCache = {
    entries: new Map(),
    totalBytes: 0,
    mountedRenderers: 0,
  };
  displayMediaCaches.set(sessionId, created);
  return created;
}

function pruneDisplayMediaCache(
  sessionCache: SessionDisplayMediaCache,
  maxBytes: number,
): void {
  while (sessionCache.totalBytes > maxBytes && sessionCache.entries.size > 0) {
    const oldestKey = sessionCache.entries.keys().next().value;
    if (!oldestKey) {
      break;
    }

    const entry = sessionCache.entries.get(oldestKey);
    if (!entry) {
      sessionCache.entries.delete(oldestKey);
      continue;
    }

    sessionCache.totalBytes -= entry.size;
    sessionCache.entries.delete(oldestKey);
  }
}

function getDisplayMediaCacheEntry(
  sessionId: string,
  uri: string,
): DisplayMediaCacheEntry | undefined {
  return displayMediaCaches.get(sessionId)?.entries.get(uri);
}

function touchDisplayMediaCacheEntry(
  sessionId: string,
  uri: string,
): DisplayMediaCacheEntry | undefined {
  const sessionCache = displayMediaCaches.get(sessionId);
  if (!sessionCache) {
    return undefined;
  }

  const cached = sessionCache.entries.get(uri);
  if (!cached) {
    return undefined;
  }

  sessionCache.entries.delete(uri);
  sessionCache.entries.set(uri, cached);
  return cached;
}

function updateDisplayMediaCache(
  sessionId: string,
  uri: string,
  url: string,
  size: number,
): void {
  const sessionCache = getSessionDisplayMediaCache(sessionId);
  const existing = sessionCache.entries.get(uri);
  if (existing) {
    sessionCache.totalBytes -= existing.size;
    sessionCache.entries.delete(uri);
  }

  sessionCache.entries.set(uri, { url, size });
  sessionCache.totalBytes += size;
  pruneDisplayMediaCache(sessionCache, DISPLAY_MEDIA_CACHE_MAX_BYTES);
}

function retainDisplayMediaCacheSession(sessionId: string): void {
  const sessionCache = getSessionDisplayMediaCache(sessionId);
  sessionCache.mountedRenderers += 1;
}

function releaseDisplayMediaCacheSession(sessionId: string): void {
  const sessionCache = displayMediaCaches.get(sessionId);
  if (!sessionCache) {
    return;
  }

  sessionCache.mountedRenderers = Math.max(
    0,
    sessionCache.mountedRenderers - 1,
  );
  if (sessionCache.mountedRenderers === 0) {
    displayMediaCaches.delete(sessionId);
  }
}

function inlineMediaToDataUrl(rawData: string, mimeType: string): string {
  return rawData.startsWith('data:')
    ? rawData
    : `data:${mimeType};base64,${rawData}`;
}

function decodeBase64ToBytes(value: string): Uint8Array {
  const normalized = value.replace(/\s+/g, '');
  const binary = atob(normalized);
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes;
}

function dataUrlToBlob(dataUrl: string, fallbackMimeType: string): Blob {
  const separatorIndex = dataUrl.indexOf(',');
  if (separatorIndex === -1) {
    throw new Error('Invalid data URL');
  }

  const metadata = dataUrl.slice(0, separatorIndex);
  const payload = dataUrl.slice(separatorIndex + 1);
  const mimeTypeMatch = metadata.match(/^data:([^;,]+)/);
  const resolvedMimeType = mimeTypeMatch?.[1] || fallbackMimeType;

  if (metadata.includes(';base64')) {
    return new Blob([decodeBase64ToBytes(payload)], {
      type: resolvedMimeType,
    });
  }

  return new Blob([decodeURIComponent(payload)], {
    type: resolvedMimeType,
  });
}

async function resolveImageBlob(
  rawData: string | undefined,
  imageSrc: string,
  mimeType: string,
): Promise<Blob> {
  if (rawData) {
    return rawData.startsWith('data:')
      ? dataUrlToBlob(rawData, mimeType)
      : new Blob([decodeBase64ToBytes(rawData)], { type: mimeType });
  }

  if (imageSrc.startsWith('data:')) {
    return dataUrlToBlob(imageSrc, mimeType);
  }

  const response = await fetch(imageSrc);
  if (!response.ok) {
    throw new Error(`Failed to read image source: ${response.status}`);
  }

  const blob = await response.blob();
  if (blob.type) {
    return blob;
  }

  return new Blob([await blob.arrayBuffer()], { type: mimeType });
}

function getImageDownloadName(
  uri: string | undefined,
  mimeType: string,
): string {
  const uriSegment = uri?.split(/[?#]/u, 1)[0]?.split('/').pop();
  if (uriSegment && uriSegment.includes('.')) {
    return uriSegment;
  }

  const extension = mimeType.split('/')[1]?.split('+')[0] || 'png';
  return `image-${Date.now()}.${extension}`;
}

function canWriteImagesToClipboard(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    typeof navigator.clipboard?.write === 'function' &&
    typeof ClipboardItem !== 'undefined'
  );
}

function MediaLoadError({ label, detail }: { label: string; detail?: string }) {
  return (
    <div
      role="status"
      className="rounded-lg border border-dashed border-muted-foreground/30 bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
    >
      <div className="font-medium text-foreground/80">{`Failed to load ${label}`}</div>
      {detail ? <div className="mt-1 text-xs">{detail}</div> : null}
    </div>
  );
}

function isRenderItemType(item: RenderItem, type: RenderItem['type']): boolean {
  return item.type === type;
}

function useResolvedMediaSource(
  rawData: string | undefined,
  uri: string | undefined,
  mimeType: string,
  sessionId: string | undefined,
): { resolvedSrc: string | undefined; loadError?: string } {
  const [resolvedSrc, setResolvedSrc] = useState<string | undefined>(() => {
    if (rawData) {
      return inlineMediaToDataUrl(rawData, mimeType);
    }

    if (!uri) {
      return undefined;
    }

    if (!uri.startsWith('file://')) {
      return uri;
    }

    return sessionId
      ? getDisplayMediaCacheEntry(sessionId, uri)?.url
      : undefined;
  });
  const [loadError, setLoadError] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;

    if (rawData) {
      setResolvedSrc(inlineMediaToDataUrl(rawData, mimeType));
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    if (!uri) {
      setResolvedSrc(undefined);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    if (!uri.startsWith('file://')) {
      setResolvedSrc(uri);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    const cached = sessionId
      ? touchDisplayMediaCacheEntry(sessionId, uri)
      : undefined;
    if (cached) {
      setResolvedSrc(cached.url);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    setResolvedSrc(undefined);

    if (!sessionId) {
      const errorMessage = 'Cannot read local media without a session';
      logger.error('Failed to resolve display media source', {
        uri,
        mimeType,
        error: errorMessage,
      });
      setLoadError(errorMessage);
      return () => {
        cancelled = true;
      };
    }

    const activeSessionId = sessionId;

    void readLocalFileAsBase64(activeSessionId, uri)
      .then((base64) => {
        if (cancelled) {
          return;
        }

        const url = inlineMediaToDataUrl(base64, mimeType);
        updateDisplayMediaCache(
          activeSessionId,
          uri,
          url,
          estimateBase64Bytes(base64),
        );
        setResolvedSrc(url);
        setLoadError(undefined);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }

        const errorMessage =
          error instanceof Error ? error.message : String(error);
        logger.error('Failed to resolve display media source', {
          uri,
          mimeType,
          error: errorMessage,
        });
        setLoadError(errorMessage);
      });

    return () => {
      cancelled = true;
    };
  }, [mimeType, rawData, sessionId, uri]);

  return { resolvedSrc, loadError };
}

interface MediaRendererProps {
  rawData?: string;
  uri?: string;
  mimeType: string;
  itemKey: string;
  sessionId?: string;
}

function ImageContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { downloadMediaFile } = useRustBackend();
  const { resolvedSrc: imageSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );
  const [copied, setCopied] = useState(false);
  const canCopyImage = canWriteImagesToClipboard();

  if (!imageSrc) {
    return loadError ? (
      <MediaLoadError label="image" detail={loadError} />
    ) : null;
  }

  const handleCopy = async () => {
    if (!canCopyImage) {
      toast.error('Image clipboard is not supported in this environment');
      return;
    }

    try {
      const blob = await resolveImageBlob(rawData, imageSrc, mimeType);
      const clipboardMimeType = blob.type || mimeType;
      await navigator.clipboard.write([
        new ClipboardItem({
          [clipboardMimeType]: blob,
        }),
      ]);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.success('Image copied to clipboard');
    } catch (err) {
      logger.error('Failed to copy image to clipboard', err);
      toast.error('Failed to copy image to clipboard');
    }
  };

  const handleDownload = async () => {
    const fileName = getImageDownloadName(uri, mimeType);
    const dataBase64 =
      rawData && !rawData.startsWith('data:')
        ? rawData
        : imageSrc.startsWith('data:')
          ? imageSrc.slice(imageSrc.indexOf(',') + 1)
          : undefined;
    const fileUrl =
      dataBase64 === undefined && !rawData && uri?.startsWith('file://')
        ? uri
        : undefined;

    try {
      const result = await downloadMediaFile({
        sessionId,
        fileName,
        mimeType,
        dataBase64,
        fileUrl,
      });

      if (result === 'Download cancelled by user') {
        toast.info('Download cancelled');
        return;
      }

      toast.success(result);
    } catch (err) {
      logger.error('Failed to download image', err);
      toast.error('Failed to download image');
    }
  };

  return (
    <div className="group relative inline-block max-w-full">
      {/* Quick Action Buttons - Visible on hover */}
      <div className="absolute top-2 right-2 flex items-center gap-1 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity z-10 bg-background/80 backdrop-blur-sm p-1 rounded-md border border-border shadow-sm">
        <button
          type="button"
          onClick={handleCopy}
          disabled={!canCopyImage}
          className="flex items-center justify-center p-1.5 hover:bg-secondary rounded text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={
            canCopyImage
              ? 'Copy Image'
              : 'Image clipboard is not supported in this environment'
          }
          aria-label="Copy image to clipboard"
        >
          {copied ? (
            <Check size={16} className="text-emerald-500" />
          ) : (
            <Copy size={16} />
          )}
        </button>
        <button
          type="button"
          onClick={handleDownload}
          className="flex items-center justify-center p-1.5 hover:bg-secondary rounded text-muted-foreground hover:text-foreground transition-colors"
          title="Download Image"
          aria-label="Download image"
        >
          <Download size={16} />
        </button>
      </div>

      <img
        key={itemKey}
        src={imageSrc}
        alt="Tool output"
        className="max-w-full h-auto rounded-lg shadow-sm border border-border/10"
      />
    </div>
  );
}

function AudioContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { resolvedSrc: audioSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );

  if (!audioSrc) {
    return loadError ? (
      <MediaLoadError label="audio" detail={loadError} />
    ) : null;
  }

  return (
    <audio key={itemKey} controls className="w-full">
      <source src={audioSrc} type={mimeType} />
      Your browser does not support the audio element.
    </audio>
  );
}

function VideoContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { resolvedSrc: videoSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );

  if (!videoSrc) {
    return loadError ? (
      <MediaLoadError label="video" detail={loadError} />
    ) : null;
  }

  return (
    <video key={itemKey} controls className="w-full rounded-lg shadow-sm">
      <source src={videoSrc} type={mimeType} />
      Your browser does not support the video element.
    </video>
  );
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
      autoResizeIframe: { height: true, width: false }, // Width resize can feed iframe layout loops for raw HTML resources
      style: { height: '384px', maxHeight: '80vh' }, // Capped at 80vh to prevent infinite growth and improve UX
      iframeProps: {
        className: 'w-full',
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

  // V2 UI Focus: If a UI resource is present, filter out regular text blocks
  // to ensure the interactive UI remains the focal point without redundant markdown text.
  const hasUIResource = useMemo(
    () => renderItems.some((item) => isRenderItemType(item, 'resource')),
    [renderItems],
  );

  const displayItems = useMemo(
    () =>
      hasUIResource
        ? renderItems.filter((item) => !isRenderItemType(item, 'text'))
        : renderItems,
    [hasUIResource, renderItems],
  );

  return (
    <div className={`flex flex-col gap-2 min-w-0 max-w-full ${className}`}>
      {displayItems.map((item, index) => {
        const key = `${message?.id}_${index}`;

        // Handle specialized tool groups
        if (isRenderItemType(item, 'tool_group_block')) {
          const groupBlock = item as ToolGroupBlock;

          return (
            <div key={key} className="my-2">
              <AgentToolGroupBlock
                message={
                  message ||
                  ({
                    id: 'dummy',
                    role: 'assistant',
                    content: [],
                  } as unknown as Message)
                }
                groupBlock={groupBlock}
                toolResultsMap={toolResultsMap}
                isLast={index === displayItems.length - 1}
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
                hideCopyButton={message?.role === 'tool'}
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
              uri?: string;
              mimeType?: string;
            };
            const rawData = imageItem.data || imageItem.source?.data;
            const uri = imageItem.uri || imageItem.source?.uri;
            const mimeType = imageItem.mimeType || 'image/png';
            return (
              <ImageContentRenderer
                key={itemKey}
                itemKey={itemKey}
                rawData={rawData}
                uri={uri}
                mimeType={mimeType}
                sessionId={message?.sessionId}
              />
            );
          }
          case 'audio': {
            const audioItem = contentItem as {
              data?: string;
              source?: { data?: string; uri?: string };
              uri?: string;
              mimeType?: string;
            };
            const mimeType = audioItem.mimeType || 'audio/mpeg';
            const rawData = audioItem.data || audioItem.source?.data;
            const uri = audioItem.uri || audioItem.source?.uri;
            return (
              <AudioContentRenderer
                key={itemKey}
                itemKey={itemKey}
                rawData={rawData}
                uri={uri}
                mimeType={mimeType}
                sessionId={message?.sessionId}
              />
            );
          }
          case 'video': {
            const videoItem = contentItem as {
              data?: string;
              source?: { data?: string; uri?: string };
              uri?: string;
              mimeType?: string;
            };
            const mimeType = videoItem.mimeType || 'video/mp4';
            const rawData = videoItem.data || videoItem.source?.data;
            const uri = videoItem.uri || videoItem.source?.uri;
            return (
              <VideoContentRenderer
                key={itemKey}
                itemKey={itemKey}
                rawData={rawData}
                uri={uri}
                mimeType={mimeType}
                sessionId={message?.sessionId}
              />
            );
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
