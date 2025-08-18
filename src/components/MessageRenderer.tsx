import React from 'react';
import type { MCPContent } from '@/lib/mcp-types';
import UIResourceRenderer from './ui/UIResourceRenderer';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MessageRenderer');

interface MessageRendererProps {
  content: string | MCPContent[];
  className?: string;
}

export const MessageRenderer: React.FC<MessageRendererProps> = ({
  content,
  className = '',
}) => {
  const { openExternalUrl } = useRustBackend();

  const handleLinkClick = async (e: React.MouseEvent, url: string) => {
    e.preventDefault();
    logger.info('Opening external URL', { url });
    
    try {
      await openExternalUrl(url);
      logger.info('External URL opened successfully', { url });
    } catch (error) {
      logger.error('Failed to open external URL via Tauri, falling back to window.open', { url, error });
      // Fallback for browser environment
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };
  if (typeof content === 'string') {
    return <div className={`message-text ${className}`}>{content}</div>;
  }

  return (
    <div className={`message-content ${className}`}>
      {content.map((item, index) => {
        switch (item.type) {
          case 'text':
            return (
              <div key={index} className="content-text">
                {(item as { text: string }).text}
              </div>
            );
          case 'resource':
            return (
              <UIResourceRenderer
                key={index}
                resource={
                  (
                    item as {
                      resource: {
                        uri?: string;
                        mimeType: string;
                        text?: string;
                        blob?: string;
                      };
                    }
                  ).resource
                }
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
                className="content-image max-w-full h-auto"
              />
            ) : null;
          }
          case 'audio': {
            const audioItem = item as { data?: string; mimeType?: string };
            return audioItem.data ? (
              <audio key={index} controls className="content-audio">
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
              <div key={index} className="content-resource-link">
                <a
                  href={linkItem.uri}
                  onClick={(e) => handleLinkClick(e, linkItem.uri)}
                  className="text-blue-600 hover:text-blue-800 underline cursor-pointer"
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
              <div key={index} className="content-unknown text-gray-500">
                [{'type' in item ? (item as { type: string }).type : 'unknown'}]
              </div>
            );
        }
      })}
    </div>
  );
};

export default MessageRenderer;
