import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSessionContext } from '../context/SessionContext';
import { useSessionHistory } from '../context/SessionHistoryContext';
import { useAIService } from './use-ai-service';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';
import { useSettings } from './use-settings';

const logger = getLogger('useChatContext');

interface ChatContextReturn {
  submit: (messageToAdd?: Message[]) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
}

const validateMessage = (message: Message): boolean => {
  return Boolean(message.role && (message.content || message.tool_calls));
};

export const useChatContext = (): ChatContextReturn => {
  const { messages: history, addMessage } = useSessionHistory();
  const { submit: triggerAIService, isLoading, response } = useAIService();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  
  const [streamingMessage, setStreamingMessage] = useState<Message | null>(null);
  
  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Combine history with streaming message, avoiding duplicates
  const messages = useMemo(() => {
    if (!streamingMessage) {
      return history;
    }

    // Check if streaming message already exists in history as finalized
    const existsInHistory = history.some(
      (message) => message.id === streamingMessage.id && !message.isStreaming
    );

    return existsInHistory ? history : [...history, streamingMessage];
  }, [streamingMessage, history]);

  // Handle AI service streaming responses
  useEffect(() => {
    if (!response) return;

    setStreamingMessage((previous) => {
      if (previous) {
        // Merge response with existing streaming message
        return { ...previous, ...response };
      }

      // Create new streaming message with proper defaults
      return {
        ...response,
        id: response.id ?? createId(),
        content: response.content ?? '',
        role: 'assistant' as const,
        sessionId: response.sessionId ?? currentSession?.id ?? '',
        isStreaming: response.isStreaming !== false,
      };
    });
  }, [response, currentSession?.id]);

  // Clear streaming state when message is finalized in history
  useEffect(() => {
    if (!streamingMessage || streamingMessage.isStreaming) return;

    const isMessageInHistory = history.some(
      (message) => message.id === streamingMessage.id && !message.isStreaming
    );

    if (isMessageInHistory) {
      logger.info('Message finalized in history, clearing streaming state', {
        messageId: streamingMessage.id,
      });
      setStreamingMessage(null);
    }
  }, [history, streamingMessage]);

  const submit = useCallback(
    async (messageToAdd?: Message[]): Promise<Message> => {
      if (!currentSession) {
        throw new Error('No active session available for message submission');
      }

      try {
        let messagesToSend = messages;

        // Process and validate new messages if provided
        if (messageToAdd?.length) {
          const processedMessages = await Promise.all(
            messageToAdd.map(async (message) => {
              if (!validateMessage(message)) {
                throw new Error(
                  'Invalid message: must have role and either content or tool_calls'
                );
              }

              const messageWithSession = {
                ...message,
                sessionId: currentSession.id,
              };

              await addMessage(messageWithSession);
              return messageWithSession;
            })
          );

          messagesToSend = [...messages, ...processedMessages];
        }

        // Send windowed messages to AI service
        const aiResponse = await triggerAIService(
          messagesToSend.slice(-messageWindowSize)
        );

        // Handle AI response persistence
        if (aiResponse) {
          const finalizedMessage: Message = {
            ...aiResponse,
            isStreaming: false,
            sessionId: currentSession.id,
          };

          logger.info('Finalizing AI response', {
            messageId: finalizedMessage.id,
          });

          // Update streaming state and persist to history
          setStreamingMessage(finalizedMessage);
          await addMessage(finalizedMessage);
        }

        return aiResponse;
      } catch (error) {
        logger.error('Message submission failed', { error });
        setStreamingMessage(null);
        throw error;
      }
    },
    [
      currentSession,
      messages,
      messageWindowSize,
      triggerAIService,
      addMessage,
    ]
  );

  return {
    submit,
    isLoading,
    messages,
  };
};