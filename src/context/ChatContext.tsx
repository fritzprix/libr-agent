import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useSessionContext } from './SessionContext';
import { useSessionHistory } from './SessionHistoryContext';
import { useAIService } from '../hooks/use-ai-service';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';
import { useSettings } from '../hooks/use-settings';
import { useAssistantExtension } from './AssistantExtensionContext';

const logger = getLogger('ChatContext');

interface ChatContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

const validateMessage = (message: Message): boolean => {
  return Boolean(message.role && (message.content || message.tool_calls));
};

interface ChatProviderProps {
  children: React.ReactNode;
}

export function ChatProvider({ children }: ChatProviderProps) {
  const { messages: history, addMessage } = useSessionHistory();
  const { submit: triggerAIService, isLoading, response } = useAIService();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  const { getExtensionSystemPrompts } = useAssistantExtension();

  const [streamingMessage, setStreamingMessage] = useState<Message | null>(
    null,
  );

  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Create system message from prompt
  const createSystemMessage = useCallback((prompt: string, agentKey: string): Message => ({
    id: `system-${agentKey}-${createId()}`,
    role: 'system',
    content: prompt,
    sessionId: currentSession?.id ?? '',
    isStreaming: false,
    createdAt: new Date(),
  }), [currentSession?.id]);

  // Get active system prompts sorted by priority (only from extensions)
  const getActiveSystemPrompts = useCallback(() => {
    return getExtensionSystemPrompts();
  }, [getExtensionSystemPrompts]);

  // Combine history with streaming message, avoiding duplicates
  const messages = useMemo(() => {
    if (!streamingMessage) {
      return history;
    }

    // Check if streaming message already exists in history as finalized
    const existingMessage = history.find(
      (message) => message.id === streamingMessage.id && !message.isStreaming,
    );

    return existingMessage
      ? history.map((msg) =>
          msg.id === streamingMessage.id
            ? { ...msg, ...streamingMessage }
            : msg,
        )
      : [...history, streamingMessage];
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
      (message) => message.id === streamingMessage.id && !message.isStreaming,
    );

    if (isMessageInHistory) {
      logger.info('Message finalized in history, clearing streaming state', {
        messageId: streamingMessage.id,
      });
      setStreamingMessage(null);
    }
  }, [history, streamingMessage]);

  const submit = useCallback(
    async (messageToAdd?: Message[], agentKey?: string): Promise<Message> => {
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
                  'Invalid message: must have role and either content or tool_calls',
                );
              }

              const messageWithSession = {
                ...message,
                sessionId: currentSession.id,
              };

              await addMessage(messageWithSession);
              return messageWithSession;
            }),
          );

          messagesToSend = [...messages, ...processedMessages];
        }

        // Get windowed messages (excluding system prompts from history)
        const userMessages = messagesToSend
          .filter((msg) => msg.role !== 'system')
          .slice(-messageWindowSize);

        // Prepare system prompts for this submission
        const activePrompts = getActiveSystemPrompts();
        const systemMessages = activePrompts.map((promptData) =>
          createSystemMessage(promptData.prompt, promptData.key)
        );

        // Combine system prompts with user messages
        const finalMessages = [...systemMessages, ...userMessages];

        logger.debug('Submitting messages with system prompts', {
          systemPromptsCount: systemMessages.length,
          userMessagesCount: userMessages.length,
          agentKey,
        });

        // Send combined messages to AI service
        const aiResponse = await triggerAIService(finalMessages);

        // Handle AI response persistence
        if (aiResponse) {
          const finalizedMessage: Message = {
            ...aiResponse,
            isStreaming: false,
            sessionId: currentSession.id,
          };

          logger.info('Finalizing AI response', {
            messageId: finalizedMessage.id,
            agentKey,
          });

          // Update streaming state and persist to history
          setStreamingMessage(finalizedMessage);
          await addMessage(finalizedMessage);
        }

        return aiResponse;
      } catch (error) {
        logger.error('Message submission failed', { error, agentKey });
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
      getActiveSystemPrompts,
      createSystemMessage,
    ],
  );

  const value: ChatContextValue = useMemo(
    () => ({
      submit,
      isLoading,
      messages,
    }),
    [
      messages,
      submit,
      isLoading,
    ],
  );

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}

export function useChatContext(): ChatContextValue {
  const context = useContext(ChatContext);
  if (context === undefined) {
    throw new Error('useChatContext must be used within a ChatProvider');
  }
  return context;
}
