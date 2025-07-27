import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSessionContext } from '../context/SessionContext';
import { useSessionHistory } from '../context/SessionHistoryContext';
import { useAIService } from './use-ai-service';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';

const logger = getLogger('useChatContext');

interface ChatContextReturn {
  submit: (messageToAdd?: Message[]) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
}

const validateMessage = (message: Message): boolean => {
  return !!(message.role && (message.content || message.tool_calls));
};

export const useChatContext = (): ChatContextReturn => {
  const { messages: history, addMessage } = useSessionHistory();
  const { submit: triggerAIService, isLoading, response } = useAIService();
  const { current: currentSession } = useSessionContext();
  const [currentStreaming, setCurrentStreaming] = useState<Message | null>(
    null,
  );

  const messages = useMemo(() => {
    if (currentStreaming) {
      // Check if we already have this message in history (non-streaming version)
      const existingMessage = history.find(
        (m) => m.id === currentStreaming.id && !m.isStreaming,
      );
      if (existingMessage) {
        // If the message exists in history and is not streaming, use history only
        return [...history];
      }
      // Otherwise, show streaming message
      return [...history, currentStreaming];
    }
    return [...history];
  }, [currentStreaming, history]);

  useEffect(() => {
    if (response) {
      logger.info('Updating currentStreaming with response:', { response });
      setCurrentStreaming((prev) => {
        if (prev) {
          logger.info('Merging with existing streaming message:', {
            prev,
            response,
          });
          return { ...prev, ...response };
        }
        // If prev is null but we have a response, create a new streaming message
        const newStreaming = {
          ...response,
          id: response.id || createId(),
          content: response.content || '',
          role: 'assistant' as const,
          sessionId: response.sessionId || currentSession?.id || '',
          isStreaming: response.isStreaming !== false,
        };
        logger.info('Creating new streaming message:', { newStreaming });
        return newStreaming;
      });
    }
  }, [response, currentSession?.id]);

  // Effect to clear streaming state when message appears in history
  useEffect(() => {
    if (currentStreaming && !currentStreaming.isStreaming) {
      const messageInHistory = history.find(
        (m) => m.id === currentStreaming.id && !m.isStreaming,
      );
      if (messageInHistory) {
        logger.info(
          'Final message found in history, clearing streaming state:',
          {
            messageId: currentStreaming.id,
          },
        );
        setCurrentStreaming(null);
      }
    }
  }, [history, currentStreaming]);

  const handleSubmit = useCallback(
    async (messageToAdd?: Message[]): Promise<Message> => {
      if (!currentSession) {
        throw new Error('No active session to submit messages to.');
      }

      try {
        let messagesToSend: Message[];

        if (messageToAdd) {
          // Validate and persist new messages before processing
          const messagesWithSessionId = await Promise.all(
            messageToAdd.map(async (msg) => {
              if (!validateMessage(msg)) {
                throw new Error(
                  'Invalid message in batch: must have role and either content or tool_calls',
                );
              }
              const messageWithId = { ...msg, sessionId: currentSession.id };
              addMessage(messageWithId);
              return messageWithId;
            }),
          );

          messagesToSend = [...messages, ...messagesWithSessionId];
        } else {
          messagesToSend = messages;
        }
        const aiResponse = await triggerAIService(messagesToSend);

        if (aiResponse) {
          const responseWithSessionId: Message = {
            ...aiResponse,
            isStreaming: false,
            sessionId: currentSession.id,
          };
          logger.info('Finalizing streaming message and adding to history:', {
            responseWithSessionId,
          });

          // Update the current streaming message to be non-streaming
          setCurrentStreaming(responseWithSessionId);

          // Add to history - the useEffect will clear streaming state when it appears
          await addMessage(responseWithSessionId);
        }

        return aiResponse;
      } catch (error) {
        console.error('Failed to submit messages:', error);
        // Clear streaming state immediately on error
        setCurrentStreaming(null);
        throw error;
      }
    },
    [triggerAIService, currentSession, messages, addMessage],
  );

  const value = useMemo<ChatContextReturn>(
    () => ({
      submit: handleSubmit,
      isLoading,
      messages,
    }),
    [handleSubmit, isLoading, messages],
  );

  return value;
};
