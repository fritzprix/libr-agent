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
  addMessages: (messages: Message[]) => Promise<Message[]>;
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
      revalidateOnFocus: false, // Prevent automatic refetch on focus to avoid race conditions
      revalidateOnReconnect: true, // Keep network reconnection revalidation
      revalidateIfStale: false, // Manual refresh only to prevent message loss
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

  const addMessages = useCallback(
    (messagesToAdd: Message[]): Promise<Message[]> => {
      if (!currentSession) throw new Error('No active session.');
      messagesToAdd.forEach(validateMessage);
      const messagesWithSessionId = messagesToAdd.map((m) => ({
        ...m,
        sessionId: currentSession.id,
      }));

      const previousData = data;

      return mutate(
        (currentData) => {
          if (!currentData || currentData.length === 0) {
            return [
              {
                items: messagesWithSessionId,
                page: 1,
                pageSize: PAGE_SIZE,
                totalItems: messagesWithSessionId.length,
                hasNextPage: false,
                hasPreviousPage: false,
              },
            ];
          }
          const newData = [...currentData];
          const last = { ...newData[newData.length - 1] };
          last.items = [...last.items, ...messagesWithSessionId];
          newData[newData.length - 1] = last;
          return newData;
        },
        { revalidate: false },
      )
        .then(() => {
          return dbService.messages.upsertMany(messagesWithSessionId);
        })
        .then(() => {
          return messagesWithSessionId;
        })
        .catch((e) => {
          mutate(previousData, { revalidate: false });
          throw e;
        });
    },
    [currentSession, mutate, data, validateMessage],
  );

  // 기존 addMessage는 내부적으로 addMessages([message]) 호출
  const addMessage = useCallback(
    (message: Message): Promise<Message> => {
      return addMessages([message]).then(([added]) => added);
    },
    [addMessages],
  );

  const updateMessage = useCallback(
    (messageId: string, updates: Partial<Message>): Promise<void> => {
      if (!currentSession) throw new Error('No active session.');

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      return mutate(
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
      )
        .then(() => {
          const existing = messages.find((m) => m.id === messageId);
          if (!existing) throw new Error(`Message ${messageId} not found.`);
          return dbService.messages.upsert({ ...existing, ...updates });
        })
        .catch((e) => {
          logger.error('Failed to update message, rolling back', e);
          // 실제 롤백: 이전 데이터로 복원
          mutate(previousData, { revalidate: false });
          throw e;
        });
    },
    [currentSession, mutate, messages, data],
  );

  const deleteMessage = useCallback(
    (messageId: string): Promise<void> => {
      if (!currentSession) throw new Error('No active session.');

      // 낙관적 업데이트 전 현재 데이터 백업
      const previousData = data;

      return mutate(
        (currentData) => {
          if (!currentData) return [];
          return currentData.map((page) => ({
            ...page,
            items: page.items.filter((msg) => msg.id !== messageId),
          }));
        },
        { revalidate: false },
      )
        .then(() => {
          return dbService.messages.delete(messageId);
        })
        .catch((e) => {
          logger.error('Failed to delete message, rolling back', e);
          // 실제 롤백: 이전 데이터로 복원
          mutate(previousData, { revalidate: false });
          throw e;
        });
    },
    [currentSession, mutate, data],
  );

  const clearHistory = useCallback((): Promise<void> => {
    if (!currentSession) throw new Error('No active session.');

    // 낙관적 업데이트 전 현재 데이터 백업
    const previousData = data;

    return new Promise<void>((resolve, reject) => {
      mutate(() => [], { revalidate: false })
        .then(() => {
          dbUtils
            .deleteAllMessagesForSession(currentSession.id)
            .then(() => {
              resolve();
            })
            .catch((e: unknown) => {
              logger.error('Failed to clear history, rolling back', e);
              // 실제 롤백: 이전 데이터로 복원
              mutate(previousData, { revalidate: false });
              reject(e);
            });
        })
        .catch(reject);
    });
  }, [currentSession, mutate, data]);

  const contextValue = useMemo(
    () => ({
      messages,
      isLoading,
      error: error ? (error as Error) : null,
      loadMore,
      hasMore,
      addMessage,
      addMessages,
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
      addMessages,
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
