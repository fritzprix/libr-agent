import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import useSWRInfinite from 'swr/infinite';
import { dbService, dbUtils, Page } from '../lib/db';
import { getLogger } from '../lib/logger';
import { useSessionContext } from './SessionContext';
import { Message } from '@/models/chat';

const logger = getLogger('SessionHistoryContext');
const PAGE_SIZE = 50;

/**
 * SessionHistoryContext의 인터페이스.
 * 여러 메시지를 한 번에 추가하는 addHistoryMessages 함수가 추가되었습니다.
 */
interface SessionHistoryContextType {
  messages: Message[];
  isLoading: boolean;
  error: Error | null;
  loadMore: () => void;
  hasMore: boolean;
  addMessage: (message: Message) => Promise<Message>;
  addHistoryMessages: (messages: Message[]) => Promise<Message[]>;
  updateMessage: (
    messageId: string,
    updates: Partial<Message>,
  ) => Promise<void>;
  deleteMessage: (messageId: string) => Promise<void>;
  clearHistory: () => Promise<void>;
}

const SessionHistoryContext = createContext<SessionHistoryContextType | null>(
  null,
);

/**
 * Custom hook to use SessionHistoryContext.
 * @throws Error if used outside of SessionHistoryProvider
 */
export function useSessionHistory(): SessionHistoryContextType {
  const context = useContext(SessionHistoryContext);
  if (!context) {
    throw new Error(
      'useSessionHistory must be used within a SessionHistoryProvider',
    );
  }
  return context;
}

/**
 * Provider component for SessionHistoryContext.
 */
export function SessionHistoryProvider({ children }: { children: ReactNode }) {
  const { current: currentSession } = useSessionContext();

  const { data, error, isLoading, setSize, mutate } = useSWRInfinite<
    Page<Message>
  >(
    (pageIndex, previousPageData) => {
      if (!currentSession?.id) return null;
      if (previousPageData && !previousPageData.hasNextPage) return null;
      return [currentSession.id, 'messages', pageIndex + 1];
    },
    async ([sessionId, , page]: [string, string, number]) => {
      return dbUtils.getMessagesPageForSession(sessionId, page, PAGE_SIZE);
    },
    {
      revalidateOnFocus: true,
      revalidateOnReconnect: true,
      revalidateIfStale: true,
    },
  );

  const messages = useMemo(() => {
    return data ? data.flatMap((page) => page.items) : [];
  }, [data]);

  const hasMore = useMemo(() => {
    return data?.[data.length - 1]?.hasNextPage ?? false;
  }, [data]);

  const loadMore = useCallback(() => {
    if (!isLoading && hasMore) {
      setSize((size) => size + 1);
    }
  }, [isLoading, hasMore, setSize]);

  useEffect(() => {
    logger.info('current session : ', { currentSession });
  }, [currentSession]);

  const validateMessage = useCallback((message: Message): boolean => {
    return !!(message.role && (message.content || message.tool_calls));
  }, []);

  const addMessage = useCallback(
    async (message: Message): Promise<Message> => {
      if (!currentSession) throw new Error('No active session.');
      if (!validateMessage(message))
        throw new Error('Invalid message structure.');

      const messageWithSessionId = { ...message, sessionId: currentSession.id };

      logger.info('Adding message with optimistic update:', {
        messageWithSessionId,
      });

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      // 낙관적 업데이트
      await mutate(
        (currentData) => {
          if (!currentData || currentData.length === 0) {
            const newPage: Page<Message> = {
              items: [messageWithSessionId],
              page: 1,
              pageSize: PAGE_SIZE,
              totalItems: 1,
              hasNextPage: false,
              hasPreviousPage: false,
            };
            logger.info('Creating new page with first message:', { newPage });
            return [newPage];
          }
          const newData = [...currentData];
          const lastPage = { ...newData[newData.length - 1] };
          lastPage.items = [...lastPage.items, messageWithSessionId];
          newData[newData.length - 1] = lastPage;
          logger.info('Added message to existing page:', {
            messageId: messageWithSessionId.id,
            totalMessages: lastPage.items.length,
          });
          return newData;
        },
        { revalidate: false },
      );

      try {
        await dbService.messages.upsert(messageWithSessionId);
        logger.info('Successfully persisted message to DB:', {
          messageId: messageWithSessionId.id,
        });
      } catch (e) {
        logger.error('Failed to add message, rolling back', e);
        // 실제 롤백: 이전 데이터로 복원
        await mutate(previousData, { revalidate: false });
        throw e;
      }
      return messageWithSessionId;
    },
    [currentSession, mutate, validateMessage, data],
  );

  /**
   * 여러 메시지를 한 번에 추가하는 함수.
   * ChatContext의 submit 함수에서 사용됩니다.
   */
  const addHistoryMessages = useCallback(
    async (messagesToAdd: Message[]): Promise<Message[]> => {
      if (!currentSession) throw new Error('No active session.');
      messagesToAdd.forEach(validateMessage);

      const messagesWithSessionId = messagesToAdd.map((msg) => ({
        ...msg,
        sessionId: currentSession.id,
      }));

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      await mutate(
        (currentData) => {
          if (!currentData || currentData.length === 0) {
            const newPage: Page<Message> = {
              items: messagesWithSessionId,
              page: 1,
              pageSize: PAGE_SIZE,
              totalItems: messagesWithSessionId.length,
              hasNextPage: false,
              hasPreviousPage: false,
            };
            return [newPage];
          }
          const newData = [...currentData];
          const lastPage = { ...newData[newData.length - 1] };
          lastPage.items = [...lastPage.items, ...messagesWithSessionId];
          newData[newData.length - 1] = lastPage;
          return newData;
        },
        { revalidate: false },
      );

      try {
        await dbService.messages.upsertMany(messagesWithSessionId);
      } catch (e) {
        logger.error('Failed to add message batch, rolling back', e);
        // 실제 롤백: 이전 데이터로 복원
        await mutate(previousData, { revalidate: false });
        throw e;
      }
      return messagesWithSessionId;
    },
    [currentSession, mutate, validateMessage, data],
  );

  const updateMessage = useCallback(
    async (messageId: string, updates: Partial<Message>) => {
      if (!currentSession) throw new Error('No active session.');

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      await mutate(
        (currentData) => {
          if (!currentData) return [];
          return currentData.map((page) => ({
            ...page,
            items: page.items.map((msg) =>
              msg.id === messageId ? { ...msg, ...updates } : msg,
            ),
          }));
        },
        { revalidate: false },
      );

      try {
        const existing = messages.find((m) => m.id === messageId);
        if (!existing) throw new Error(`Message ${messageId} not found.`);
        await dbService.messages.upsert({ ...existing, ...updates });
      } catch (e) {
        logger.error('Failed to update message, rolling back', e);
        // 실제 롤백: 이전 데이터로 복원
        await mutate(previousData, { revalidate: false });
        throw e;
      }
    },
    [currentSession, mutate, messages, data],
  );

  const deleteMessage = useCallback(
    async (messageId: string) => {
      if (!currentSession) throw new Error('No active session.');

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      await mutate(
        (currentData) => {
          if (!currentData) return [];
          return currentData.map((page) => ({
            ...page,
            items: page.items.filter((msg) => msg.id !== messageId),
          }));
        },
        { revalidate: false },
      );

      try {
        await dbService.messages.delete(messageId);
      } catch (e) {
        logger.error('Failed to delete message, rolling back', e);
        // 실제 롤백: 이전 데이터로 복원
        await mutate(previousData, { revalidate: false });
        throw e;
      }
    },
    [currentSession, mutate, data],
  );

  const clearHistory = useCallback(async () => {
    if (!currentSession) throw new Error('No active session.');

    // 낙관적 업데이트 전 현재 데이터 백업
    const previousData = data;

    await mutate([], { revalidate: false });

    try {
      await dbUtils.deleteAllMessagesForSession(currentSession.id);
    } catch (e) {
      logger.error('Failed to clear history, rolling back', e);
      // 실제 롤백: 이전 데이터로 복원
      await mutate(previousData, { revalidate: false });
      throw e;
    }
  }, [currentSession, mutate, data]);

  const contextValue = useMemo(
    () => ({
      messages,
      isLoading,
      error: error ? (error as Error) : null,
      loadMore,
      hasMore,
      addMessage,
      addHistoryMessages,
      updateMessage,
      deleteMessage,
      clearHistory,
    }),
    [
      messages,
      isLoading,
      error,
      loadMore,
      hasMore,
      addMessage,
      addHistoryMessages,
      updateMessage,
      deleteMessage,
      clearHistory,
    ],
  );

  return (
    <SessionHistoryContext.Provider value={contextValue}>
      {children}
    </SessionHistoryContext.Provider>
  );
}
