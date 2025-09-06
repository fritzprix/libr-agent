import React, { useCallback } from 'react';
import type { MCPContent } from '@/lib/mcp-types';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  basicComponentLibrary,
  UIResourceRenderer,
  UIActionResult,
  remoteButtonDefinition,
  remoteTextDefinition,
  remoteCardDefinition,
} from '@mcp-ui/client';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { useChatContext } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { Message } from '@/models/chat';
import { stringToMCPContentArray } from '@/lib/utils';
import { useAssistantContext } from '@/context/AssistantContext';
import { useTheme } from 'next-themes';

const logger = getLogger('MessageRenderer');

interface MessageRendererProps {
  content: MCPContent[];
  className?: string;
}

export const MessageRenderer: React.FC<MessageRendererProps> = ({
  content,
  className = '',
}) => {
  const { openExternalUrl } = useRustBackend();
  const { executeToolCall } = useUnifiedMCP();
  const { submit } = useChatContext();
  const { getCurrentSession } = useSessionContext();
  const { getCurrent } = useAssistantContext();

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
  if (typeof content === 'string') {
    return <div className={`message-text ${className}`}>{content}</div>;
  }

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

            await executeToolCall({
              id: createId(),
              type: 'function',
              function: {
                name: result.payload.toolName,
                arguments: JSON.stringify(result.payload.params),
              },
            });
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
    [executeToolCall, handleLinkClick, submit, getCurrentSession, getCurrent],
  );

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
            logger.info('resource : ', { resource: item.resource });
            return (
              <UIResourceRenderer
                key={index}
                remoteDomProps={{
                  library: basicComponentLibrary,
                  remoteElements: [remoteButtonDefinition, remoteTextDefinition, remoteCardDefinition ],
                }}
                onUIAction={handleUIAction}
                supportedContentTypes={['remoteDom', 'rawHtml', 'externalUrl']}
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
