import type { ComponentProps, MouseEvent, MutableRefObject } from 'react';
import ReactMarkdown from 'react-markdown';
import { UIResourceRenderer } from '@mcp-ui/client';
import type { MCPContent, MCPThinkingContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { ThinkingBubble } from '../../shared';
import type { RenderItem, ToolGroupBlock } from '../types';
import { AgentToolGroupBlock } from './AgentToolGroupBlock';
import {
  AudioContentRenderer,
  ImageContentRenderer,
  VideoContentRenderer,
} from './MediaContentRenderer';
import { MarkdownText } from './MarkdownText';

const logger = getLogger('AgentMessageRenderer');

type UIResourceRendererProps = ComponentProps<typeof UIResourceRenderer>;

interface ContentItemRendererProps {
  item: RenderItem;
  itemKey: string;
  isLast: boolean;
  message?: Message;
  expandResources: boolean;
  toolResultsMap?: Map<string, Message>;
  resourceRefs: MutableRefObject<Record<string, HTMLDivElement | null>>;
  markdownComponents: ComponentProps<typeof ReactMarkdown>['components'];
  remoteDomProps: UIResourceRendererProps['remoteDomProps'];
  supportedContentTypes: UIResourceRendererProps['supportedContentTypes'];
  htmlProps: UIResourceRendererProps['htmlProps'];
  onUIAction: NonNullable<UIResourceRendererProps['onUIAction']>;
  onLinkClick: (
    event: MouseEvent<HTMLAnchorElement>,
    url: string,
  ) => Promise<void> | void;
}

type ResourceContentItem = MCPContent & {
  type: 'resource';
  resource?: {
    uri: string;
    mimeType: string;
    text?: string;
    blob?: string;
    _meta?: Record<string, unknown>;
  };
};

type ResourceLinkContentItem = MCPContent & {
  type: 'resource_link';
  uri: string;
  name: string;
  description?: string;
};

type BinaryContentItem = MCPContent & {
  data?: string;
  source?: { data?: string; uri?: string };
  uri?: string;
  mimeType?: string;
};

function getFallbackMessage(): Message {
  return {
    id: 'agent-message-renderer-fallback',
    sessionId: 'agent-message-renderer-fallback',
    threadId: 'agent-message-renderer-fallback',
    role: 'assistant',
    content: [],
  };
}

function getBinaryContentSource(item: BinaryContentItem): {
  rawData: string | undefined;
  uri: string | undefined;
} {
  return {
    rawData: item.data || item.source?.data,
    uri: item.uri || item.source?.uri,
  };
}

export function ContentItemRenderer({
  item,
  itemKey,
  isLast,
  message,
  expandResources,
  toolResultsMap,
  resourceRefs,
  markdownComponents,
  remoteDomProps,
  supportedContentTypes,
  htmlProps,
  onUIAction,
  onLinkClick,
}: ContentItemRendererProps) {
  if (item.type === 'tool_group_block') {
    const groupBlock = item as ToolGroupBlock;

    return (
      <div className="my-2">
        <AgentToolGroupBlock
          message={message || getFallbackMessage()}
          groupBlock={groupBlock}
          toolResultsMap={toolResultsMap}
          isLast={isLast}
        />
      </div>
    );
  }

  const contentItem = item as MCPContent;

  switch (contentItem.type) {
    case 'thinking': {
      const thinkingItem = contentItem as MCPThinkingContent;
      return (
        <div className="mb-2">
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
          content={textItem.text}
          components={markdownComponents}
          hideCopyButton={message?.role === 'tool'}
        />
      );
    }
    case 'resource': {
      const resourceItem = contentItem as ResourceContentItem;

      if (!resourceItem.resource) {
        logger.warn('Resource content is missing resource property', {
          item,
        });
        return null;
      }

      return (
        <div
          ref={(element) => {
            resourceRefs.current[itemKey] = element;
          }}
          className={expandResources ? 'min-h-96 w-full overflow-visible' : ''}
        >
          <UIResourceRenderer
            remoteDomProps={remoteDomProps}
            onUIAction={onUIAction}
            supportedContentTypes={supportedContentTypes}
            htmlProps={htmlProps}
            resource={resourceItem.resource}
          />
        </div>
      );
    }
    case 'image': {
      const imageItem = contentItem as BinaryContentItem;
      const { rawData, uri } = getBinaryContentSource(imageItem);
      return (
        <ImageContentRenderer
          itemKey={itemKey}
          rawData={rawData}
          uri={uri}
          mimeType={imageItem.mimeType || 'image/png'}
          sessionId={message?.sessionId}
        />
      );
    }
    case 'audio': {
      const audioItem = contentItem as BinaryContentItem;
      const { rawData, uri } = getBinaryContentSource(audioItem);
      return (
        <AudioContentRenderer
          itemKey={itemKey}
          rawData={rawData}
          uri={uri}
          mimeType={audioItem.mimeType || 'audio/mpeg'}
          sessionId={message?.sessionId}
        />
      );
    }
    case 'video': {
      const videoItem = contentItem as BinaryContentItem;
      const { rawData, uri } = getBinaryContentSource(videoItem);
      return (
        <VideoContentRenderer
          itemKey={itemKey}
          rawData={rawData}
          uri={uri}
          mimeType={videoItem.mimeType || 'video/mp4'}
          sessionId={message?.sessionId}
        />
      );
    }
    case 'resource_link': {
      const linkItem = contentItem as ResourceLinkContentItem;
      return (
        <div className="rounded-lg border bg-muted p-2">
          <a
            href={linkItem.uri}
            onClick={(event) => onLinkClick(event, linkItem.uri)}
            className="text-primary underline hover:text-primary/90"
          >
            {linkItem.name}
          </a>
          {linkItem.description ? (
            <div className="mt-1 text-sm text-muted-foreground">
              {linkItem.description}
            </div>
          ) : null}
        </div>
      );
    }
    default:
      return (
        <div className="italic text-muted-foreground">
          [
          {'type' in contentItem
            ? (contentItem as { type: string }).type
            : 'unknown'}
          ]
        </div>
      );
  }
}
