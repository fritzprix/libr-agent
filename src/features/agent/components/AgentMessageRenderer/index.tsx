import React, { memo } from 'react';
import type {
  MCPContent,
  MCPThinkingContent,
  MCPToolCallContent,
  ServiceInfo,
} from '@/lib/mcp';
import { getLogger } from '@/lib/logger';
import { AgentMessageRendererProps } from './types';
import { useIsDarkMode } from './hooks/useIsDarkMode';
import { useContentGrouping } from './hooks/useContentGrouping';
import { useUIActionHandler } from './hooks/useUIActionHandler';

import { TextContent } from './content/TextContent';
import { ResourceContent } from './content/ResourceContent';
import { ImageContent, AudioContent } from './content/MediaContent';
import { ThinkingContent } from './content/ThinkingContent';
import { ToolGroupContent } from './content/ToolGroupContent';
import { ResourceLinkContent } from './content/ResourceLinkContent';

const logger = getLogger('AgentMessageRenderer');

/**
 * AgentMessageRenderer - Agent V2 Message Renderer
 *
 * Decomposed from monolithic component.
 * Handles rendering of various MCP content types:
 * - Markdown Text
 * - Resources (UI, Files)
 * - Media (Images, Audio)
 * - Thinking Bubbles
 * - Tool Calls & Results (Grouped)
 */
const AgentMessageRendererImpl: React.FC<AgentMessageRendererProps> = ({
  content,
  message,
  className = '',
  expandResources = false,
  toolResultsMap,
}) => {
  // Detect dark mode once per component mount (or change)
  const isDark = useIsDarkMode();

  // Group content items (tool calls, thinking, etc.)
  const { renderItems, contentRef } = useContentGrouping(content, message);

  // Handle UI actions from resources
  const { handleUIAction } = useUIActionHandler(contentRef);

  if (!renderItems.length) {
    return null;
  }

  return (
    // min-w-0 is crucial for flex items to shrink below their content size, preventing overflow
    <div className={`flex flex-col gap-2 min-w-0 max-w-full ${className}`}>
      {renderItems.map((item, index) => {
        const key = `${message?.id}_${index}`;

        // Handle specialized tool groups
        if ('type' in item && item.type === 'tool_group_block') {
          const groupBlock = item as {
            type: 'tool_group_block';
            items: MCPToolCallContent[];
          };
          return (
            <ToolGroupContent
              key={key}
              items={groupBlock.items}
              message={message}
              toolResultsMap={toolResultsMap}
              isLast={index === renderItems.length - 1}
            />
          );
        }

        // Handle MCP Content
        const contentItem = item as MCPContent;
        const itemKey = `${message?.id}_${contentItem.type}_${index}`;

        switch (contentItem.type) {
          case 'thinking': {
            const thinkingItem = contentItem as MCPThinkingContent;
            return (
              <ThinkingContent
                key={itemKey}
                thinking={thinkingItem}
                isStreaming={message?.isStreaming}
              />
            );
          }
          case 'text': {
            const textItem = contentItem as { text: string };
            return (
              <TextContent
                key={itemKey}
                text={textItem.text}
                isDark={isDark}
              />
            );
          }
          case 'resource': {
            // Type narrow to extract the resource property
            const resourceItem = contentItem as {
              type: 'resource';
              resource: {
                uri: string;
                mimeType: string;
                text?: string;
                blob?: string;
                _meta?: Record<string, unknown>;
              };
              serviceInfo?: ServiceInfo;
            };

            if (!resourceItem.resource) {
              logger.warn('Resource content is missing resource property', {
                item,
              });
              return null;
            }

            return (
              <ResourceContent
                key={itemKey}
                resource={resourceItem.resource}
                expandResources={expandResources}
                onUIAction={handleUIAction}
              />
            );
          }
          case 'image': {
            const imageItem = contentItem as {
              data?: string;
              source?: { data?: string; uri?: string };
              mimeType?: string;
            };
            return <ImageContent key={itemKey} image={imageItem} />;
          }
          case 'audio': {
            const audioItem = contentItem as {
              data?: string;
              mimeType?: string;
            };
            return <AudioContent key={itemKey} audio={audioItem} />;
          }
          case 'resource_link': {
            const linkItem = contentItem as {
              uri: string;
              name: string;
              description?: string;
            };
            return <ResourceLinkContent key={itemKey} link={linkItem} />;
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

// Memoized to prevent re-renders of heavy markdown and UI resource components
export const AgentMessageRenderer = memo(AgentMessageRendererImpl);
export default AgentMessageRenderer;
