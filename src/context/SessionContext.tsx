import { createId } from "@paralleldrive/cuid2";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import useSWRInfinite from "swr/infinite";
import { dbService, Page } from "../lib/db";
import { getLogger } from "../lib/logger";
import { Assistant, Session } from "../types/chat";

const logger = getLogger("SessionContext");

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
  isLoading: boolean;
  isValidating: boolean;
  error: Error | null;
  clearError: () => void;
  retryLastOperation: () => Promise<void>;
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
      "useSessionContext must be used within SessionContextProvider",
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
  return `Conversation with ${assistants.map((a) => a.name).join(", ")}`;
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
    setSize((prev) => prev + 1);
  }, [setSize]);

  /**
   * Selects a session by its id and sets it as current.
   * @param id - The session id to select
   */
  const handleSelect = useCallback(
    (id?: string) => {
      if (id === undefined) {
        setCurrent(null);
      }
      const sessions = sessionsRef.current;
      const session = sessions.find((s) => s.id === id);
      if (session) {
        setCurrent(session);
        clearError(); // Clear any errors when successfully selecting
      }
    },
    [clearError],
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
            "At least one assistant is required to start a session.",
          );
        }

        const session: Session = {
          id: createId(),
          assistants: [...assistants],
          type: assistants.length > 1 ? "group" : "single",
          createdAt: new Date(),
          updatedAt: new Date(),
          description: generateSessionDescription(assistants, description),
          name: generateSessionName(assistants, name),
        };

        // Optimistic update
        setCurrent(session);

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
        logger.error("Failed to start new session", errorObj);
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
        // Store original state for rollback
        const originalCurrent = currentRef.current;
        const originalData = data;

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
          await dbService.sessions.delete(id);
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
        logger.error(`Failed to delete session with id ${id}`, errorObj);
        setOperationError(errorObj);
        setLastFailedOperation(() => operation);
      }
    },
    [mutate, data],
  );

  const contextValue: SessionContextType = useMemo(
    () => ({
      sessions,
      current,
      getSessions: handleGetSessions,
      getCurrentSession: handleGetCurrentSession,
      loadMore: handleLoadMore,
      select: handleSelect,
      start: handleStartNew,
      delete: handleDelete,
      isLoading,
      isValidating,
      error,
      clearError,
      retryLastOperation,
    }),
    [
      sessions,
      current,
      handleGetSessions,
      handleGetCurrentSession,
      handleLoadMore,
      handleSelect,
      handleStartNew,
      handleDelete,
      isLoading,
      isValidating,
      error,
      clearError,
      retryLastOperation,
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
