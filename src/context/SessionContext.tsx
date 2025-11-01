import { createId } from '@paralleldrive/cuid2';
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import useSWRInfinite from 'swr/infinite';
import { dbService, Page } from '../lib/db';
import { dbUtils } from '@/lib/db/service';
import { deleteContentStore } from '@/lib/rust-backend-client';
import { getLogger } from '../lib/logger';
import { Assistant, Session, Thread } from '../models/chat';
import { useAssistantContext } from './AssistantContext';

const logger = getLogger('SessionContext');

/**
 * The shape of the SessionContext, providing session management and state for consumers.
 */
interface SessionContextType {
  current: Session | null;
  getCurrentSession: () => Session | null;
  sessions: Page<Session>[];
  getSessions: () => Session[];
  loadMore: () => void;
  start: (
    assistants: Assistant[],
    description?: string,
    name?: string,
  ) => Promise<void>;
  delete: (id: string) => Promise<void>;
  select: (id?: string) => void;
  updateSession: (id: string, updates: Partial<Session>) => Promise<void>;
  isLoading: boolean;
  isValidating: boolean;
  error: Error | null;
  clearError: () => void;
  retryLastOperation: () => Promise<void>;
  hasNextPage: boolean;
  clearAllSessions: () => Promise<void>;

  /**
   * NEW: Session's top-level thread metadata (read-only).
   * Derived from current.sessionThread.
   * Returns null if no session is selected.
   */
  sessionThread: Thread | null;
}

/**
 * React context for session management. Provides access to session state and actions.
 */
const SessionContext = createContext<SessionContextType | null>(null);

/**
 * Custom hook to use SessionContext with error handling.
 * @throws Error if used outside of SessionContextProvider
 */
export function useSessionContext(): SessionContextType {
  const context = useContext(SessionContext);
  if (!context) {
    throw new Error(
      'useSessionContext must be used within SessionContextProvider',
    );
  }
  return context;
}

/**
 * Generates a session name based on assistants and custom name.
 */
function generateSessionName(
  assistants: Assistant[],
  customName?: string,
): string {
  if (customName) return customName;

  const primaryName = assistants[0].name;
  return assistants.length === 1
    ? `Conversation with ${primaryName}`
    : `Conversation with ${primaryName} + ${assistants.length - 1} others`;
}

/**
 * Generates a session description based on assistants and custom description.
 */
function generateSessionDescription(
  assistants: Assistant[],
  customDescription?: string,
): string {
  if (customDescription) return customDescription;
  return `Conversation with ${assistants.map((a) => a.name).join(', ')}`;
}

/**
 * Converts unknown error to Error object.
 */
function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

/**
 * Provider component for SessionContext. Wrap your component tree with this to enable session features.
 * @param children - React children components
 */
function SessionContextProvider({ children }: { children: ReactNode }) {
  const [current, setCurrent] = useState<Session | null>(null);
  const [operationError, setOperationError] = useState<Error | null>(null);
  const [lastFailedOperation, setLastFailedOperation] = useState<
    (() => Promise<void>) | null
  >(null);

  const { setCurrentAssistant } = useAssistantContext();
  const currentRef = useRef(current);
  const sessionsRef = useRef<Session[]>([]);

  const {
    data,
    error: fetchError,
    isLoading,
    isValidating,
    setSize,
    mutate,
  } = useSWRInfinite(
    (pageIndex) => [`session`, pageIndex],
    async ([, pageIndex]) => {
      return dbService.sessions.getPage(pageIndex, 10);
    },
  );

  const sessions = useMemo(() => data ?? [], [data]);

  const hasNextPage = useMemo(
    () => !(sessions.length > 0 && !sessions[sessions.length - 1].hasNextPage),
    [sessions],
  );

  // Combined error state
  const error = useMemo(() => {
    if (fetchError) return toError(fetchError);
    return operationError;
  }, [fetchError, operationError]);

  useEffect(() => {
    currentRef.current = current;
  }, [current]);

  useEffect(() => {
    sessionsRef.current = sessions.flatMap((page) => page.items);
  }, [sessions]);

  /**
   * Clears any current error state.
   */
  const clearError = useCallback(() => {
    setOperationError(null);
    setLastFailedOperation(null);
  }, []);

  /**
   * Retries the last failed operation.
   */
  const retryLastOperation = useCallback(async () => {
    if (!lastFailedOperation) return;

    try {
      await lastFailedOperation();
      setLastFailedOperation(null);
      setOperationError(null);
    } catch (error) {
      setOperationError(toError(error));
    }
  }, [lastFailedOperation]);

  /**
   * Returns the current list of sessions (flat array).
   */
  const handleGetSessions = useCallback(() => {
    return sessionsRef.current;
  }, []);

  /**
   * Returns the currently selected session, or null if none.
   */
  const handleGetCurrentSession = useCallback(() => {
    return currentRef.current;
  }, []);

  /**
   * Loads more sessions (pagination).
   */
  const handleLoadMore = useCallback(() => {
    if (hasNextPage) {
      setSize((prev) => prev + 1);
    }
  }, [setSize, hasNextPage]);

  /**
   * Selects a session by its id and sets it as current.
   * @param id - The session id to select
   */
  const handleSelect = useCallback(
    (id?: string) => {
      // Consumers should react to session changes via context state,
      // not global window events. Remove legacy broadcast.

      if (id === undefined) {
        setCurrent(null);
        // Session backend management is now handled by BuiltInToolProvider
        return;
      }

      const sessions = sessionsRef.current;
      const session = sessions.find((s) => s.id === id);
      if (session) {
        setCurrent(session);
        if (session.type === 'single') {
          setCurrentAssistant(session.assistants[0]);
        }
        clearError(); // Clear any errors when successfully selecting

        // Session backend management is now handled by BuiltInToolProvider
      }
    },
    [clearError, setCurrentAssistant],
  );

  /**
   * Starts a new session with the given assistants, description, and name.
   * @param assistants - Array of Assistant objects (at least one required)
   * @param description - Optional session description
   * @param name - Optional session name
   */
  const handleStartNew = useCallback(
    async (assistants: Assistant[], description?: string, name?: string) => {
      const operation = async () => {
        if (!assistants.length) {
          throw new Error(
            'At least one assistant is required to start a session.',
          );
        }

        const sessionId = createId();

        // Create top thread with id === sessionId
        const sessionThread = {
          id: sessionId,
          sessionId,
          assistantId: assistants[0].id,
          createdAt: new Date(),
        };

        const session: Session = {
          id: sessionId,
          assistants: [...assistants],
          type: assistants.length > 1 ? 'group' : 'single',
          createdAt: new Date(),
          updatedAt: new Date(),
          description: generateSessionDescription(assistants, description),
          name: generateSessionName(assistants, name),
          sessionThread, // Include top thread
        };

        logger.info('Creating new session with top thread', {
          sessionId: session.id,
          topThreadId: sessionThread.id,
          assistants: assistants.map((a) => a.name),
          type: session.type,
          description: session.description,
          name: session.name,
        });

        // Optimistic update
        setCurrent(session);

        // Session backend management is now handled by BuiltInToolProvider

        // Add to sessions list optimistically
        mutate(
          (currentData) => {
            if (!currentData?.length) {
              return [
                {
                  items: [session],
                  page: 0,
                  pageSize: 10,
                  totalItems: 1,
                  totalPages: 1,
                  hasNextPage: false,
                  hasPreviousPage: false,
                },
              ];
            }
            const updatedData = [...currentData];
            updatedData[0] = {
              ...updatedData[0],
              items: [session, ...updatedData[0].items],
              totalItems: updatedData[0].totalItems + 1,
            };
            return updatedData;
          },
          false, // Don't revalidate immediately
        );

        try {
          await dbService.sessions.upsert(session);
          // No need to revalidate - optimistic update contains all necessary data
        } catch (error) {
          // Rollback optimistic update
          setCurrent(null);
          await mutate();
          throw error;
        }
      };

      try {
        await operation();
        setOperationError(null);
        setLastFailedOperation(null);
      } catch (error) {
        const errorObj = toError(error);
        logger.error('Failed to start new session', errorObj);
        setOperationError(errorObj);
        setLastFailedOperation(() => operation);
      }
    },
    [mutate],
  );

  /**
   * Deletes a session by its id. If the deleted session is current, clears current.
   * @param id - The session id to delete
   */
  const handleDelete = useCallback(
    async (id: string) => {
      const operation = async () => {
        // Optimistic updates
        if (id === currentRef.current?.id) {
          setCurrent(null);
        }

        // Remove from sessions list optimistically
        mutate(
          (currentData) =>
            currentData?.map((page) => ({
              ...page,
              items: page.items.filter((s) => s.id !== id),
              totalItems: Math.max(0, page.totalItems - 1),
            })),
          false, // Don't revalidate immediately
        );

        try {
          // Remove backend content-store artifacts first (best-effort)
          try {
            await deleteContentStore(id);
          } catch (e) {
            logger.warn('deleteContentStore failed for session ' + id, e);
          }

          // Clear DB artifacts and native workspace (best-effort)
          try {
            await dbUtils.clearSessionAndWorkspace(id);
          } catch (e) {
            logger.warn('clearSessionAndWorkspace failed for session ' + id, e);
          }

          // No need to revalidate - optimistic update is accurate
        } catch (error) {
          // Unexpected error: log but do not rollback (best-effort mode)
          logger.error(`Unexpected error while deleting session ${id}`, error);
        }
      };

      try {
        await operation();
        setOperationError(null);
        setLastFailedOperation(null);
      } catch (error) {
        const errorObj = toError(error);
        logger.error(`Failed to delete session with id ${id}`, errorObj);
        setOperationError(errorObj);
        setLastFailedOperation(() => operation);
      }
    },
    [mutate, data],
  );

  /**
   * Updates a session with the provided updates.
   * @param id - The session id to update
   * @param updates - Partial session object with fields to update
   */
  const handleUpdateSession = useCallback(
    async (id: string, updates: Partial<Session>) => {
      const operation = async () => {
        // Find the session to update
        const sessions = sessionsRef.current;
        const sessionToUpdate = sessions.find((s) => s.id === id);

        if (!sessionToUpdate) {
          throw new Error(`Session with id ${id} not found`);
        }

        const updatedSession: Session = {
          ...sessionToUpdate,
          ...updates,
          updatedAt: new Date(),
        };

        // Store original state for rollback
        const originalCurrent = currentRef.current;
        const originalData = data;

        // Optimistic updates
        if (id === currentRef.current?.id) {
          setCurrent(updatedSession);
        }

        // Update in sessions list optimistically
        mutate(
          (currentData) =>
            currentData?.map((page) => ({
              ...page,
              items: page.items.map((s) => (s.id === id ? updatedSession : s)),
            })),
          false, // Don't revalidate immediately
        );

        try {
          await dbService.sessions.upsert(updatedSession);
          // No need to revalidate - optimistic update is accurate
        } catch (error) {
          // Rollback optimistic updates
          setCurrent(originalCurrent);
          mutate(originalData, false);
          throw error;
        }
      };

      try {
        await operation();
        setOperationError(null);
        setLastFailedOperation(null);
      } catch (error) {
        const errorObj = toError(error);
        logger.error(`Failed to update session with id ${id}`, errorObj);
        setOperationError(errorObj);
        setLastFailedOperation(() => operation);
      }
    },
    [mutate, data],
  );

  /**
   * Clears all sessions, messages and workspace stores.
   * Performs optimistic UI update and rolls back on failure.
   */
  const handleClearAllSessions = useCallback(async () => {
    const operation = async () => {
      // Save original data for rollback
      const originalData = data;
      const originalCurrent = currentRef.current;

      // Optimistic: clear current and sessions in UI
      setCurrent(null);
      try {
        // Collect existing session ids so we can attempt to remove native workspaces
        const sessions = await dbUtils.getAllSessions();

        // Clear sessions/messages in DB in one operation first to ensure any
        // concurrent SWR revalidation will see an empty DB.
        await dbUtils.clearAllSessions();

        // Update UI to reflect cleared DB
        await mutate([], false);

        // Attempt native workspace removal for each previously-known session id
        for (const s of sessions) {
          try {
            await deleteContentStore(s.id);
          } catch (e) {
            logger.warn('deleteContentStore failed for session ' + s.id, e);
          }
        }
      } catch (e) {
        // Rollback optimistic updates
        setCurrent(originalCurrent);
        await mutate(originalData, false);
        throw e;
      }
    };

    try {
      await operation();
      setOperationError(null);
      setLastFailedOperation(null);
    } catch (error) {
      const errorObj = toError(error);
      logger.error('Failed to clear all sessions', errorObj);
      setOperationError(errorObj);
      setLastFailedOperation(() => operation);
      throw errorObj;
    }
  }, [data, mutate]);

  // Derive sessionThread from current session
  const sessionThread = useMemo(
    () => current?.sessionThread ?? null,
    [current],
  );

  const contextValue: SessionContextType = useMemo(
    () => ({
      sessions,
      current,
      sessionThread,
      getSessions: handleGetSessions,
      getCurrentSession: handleGetCurrentSession,
      loadMore: handleLoadMore,
      select: handleSelect,
      start: handleStartNew,
      delete: handleDelete,
      updateSession: handleUpdateSession,
      clearAllSessions: handleClearAllSessions,
      isLoading,
      isValidating,
      error,
      clearError,
      retryLastOperation,
      hasNextPage,
    }),
    [
      sessions,
      current,
      sessionThread,
      handleGetSessions,
      handleGetCurrentSession,
      handleLoadMore,
      handleSelect,
      handleStartNew,
      handleDelete,
      handleUpdateSession,
      handleClearAllSessions,
      isLoading,
      isValidating,
      error,
      clearError,
      retryLastOperation,
      hasNextPage,
    ],
  );

  return (
    <SessionContext.Provider value={contextValue}>
      {children}
    </SessionContext.Provider>
  );
}

/**
 * Exported provider for SessionContext. Use to wrap your app for session features.
 */
export { SessionContextProvider };
