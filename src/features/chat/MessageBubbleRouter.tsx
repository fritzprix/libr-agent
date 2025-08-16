import { Message } from '@/models/chat';
import React, { useCallback } from 'react';
import ContentBubble from './ContentBubble';
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';
import UIResourceRenderer, {
  UIAction,
} from '@/components/ui/UIResourceRenderer';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import { useChatContext } from '@/context/ChatContext';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MessageBubbleRouter');

interface MessageBubbleRouterProps {
  message: Message;
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
}) => {
  const { executeToolCall } = useUnifiedMCP();
  const { submit } = useChatContext();

  const handleUIAction = useCallback(
    async (action: UIAction) => {
      logger.info('Handling UI action:', action);

      try {
        switch (action.type) {
          case 'tool': {
            // Convert UI action to tool call format
            const toolCall = {
              id: `ui-action-${Date.now()}`,
              type: 'function' as const,
              function: {
                name: action.payload.toolName,
                arguments: JSON.stringify(action.payload.params),
              },
            };

            // Execute the tool call
            const response = await executeToolCall(toolCall);

            // Submit the result as a new message
            // This will be processed by ToolCaller if needed
            logger.info('UI action tool call completed:', {
              toolName: action.payload.toolName,
              response,
            });
            break;
          }

          case 'intent': {
            logger.info('UI intent action:', action.payload);
            // Handle intent actions - could be extended based on specific needs
            break;
          }

          case 'prompt': {
            // Submit user prompt
            await submit([
              {
                id: `ui-prompt-${Date.now()}`,
                sessionId: message.sessionId,
                role: 'user' as const,
                content: action.payload.prompt,
              },
            ]);
            break;
          }

          case 'notify': {
            // Show notification - could use toast or similar
            logger.info('UI notification:', action.payload.message);
            break;
          }

          case 'link': {
            // Open external link
            window.open(action.payload.url, '_blank', 'noopener,noreferrer');
            break;
          }

          default:
            logger.warn('Unknown UI action type:', action);
        }
      } catch (error) {
        logger.error('Failed to handle UI action:', error);
      }
    },
    [executeToolCall, submit, message.sessionId],
  );

  // Check for UIResource first - highest priority
  if (message.uiResource) {
    return (
      <UIResourceRenderer
        resource={message.uiResource}
        onUIAction={handleUIAction}
      />
    );
  }

  if (
    message.tool_calls &&
    Array.isArray(message.tool_calls) &&
    message.tool_calls.length > 0 &&
    message.tool_calls.every((tc) => tc && tc.function && tc.function.name)
  ) {
    return <ToolCallBubble tool_calls={message.tool_calls} />;
  }

  if (message.role === 'tool') {
    return <ToolOutputBubble message={message} />;
  }

  return <ContentBubble message={message} />;
};

export default MessageBubbleRouter;
