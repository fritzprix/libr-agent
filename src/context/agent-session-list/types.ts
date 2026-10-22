import type { AgentSession, CreateSessionParams } from '@/models/agent';

export interface AgentSessionListStateContextValue {
  sessions: AgentSession[];
  notificationSessions: AgentSession[];
  unreadNotificationCount: number;
  isSessionsListLoading: boolean;
  hasMoreSessions: boolean;
  isLoadingMoreSessions: boolean;
  /** Parent session IDs currently fetching direct children for tree expand. */
  loadingChildrenParentIds: ReadonlySet<string>;
}

export interface AgentSessionListActionsContextValue {
  /**
   * Create a new agent session
   */
  createSession: (params: CreateSessionParams) => Promise<AgentSession>;

  /**
   * Load the first page of recent agent sessions.
   */
  loadSessions: (forceRefresh?: boolean) => Promise<void>;

  /**
   * Load the next page of session history.
   */
  loadMoreSessions: () => Promise<void>;

  /**
   * Lazy-load direct child sessions for a parent (tree expand).
   * Merges results into the shared session list; concurrent calls for the
   * same parent share one in-flight request.
   */
  ensureChildrenLoaded: (parentSessionId: string) => Promise<void>;

  /**
   * Delete an agent session
   */
  deleteSession: (sessionId: string) => Promise<void>;

  /**
   * Delete only this session, orphaning its direct children as top-level sessions
   */
  deleteSessionOnly: (sessionId: string) => Promise<void>;

  /**
   * Toggle the bookmark flag on a session (optimistic update).
   * Pass `currentBookmarked` when the session may not be in the browse list
   * (e.g. visible only via server search results).
   */
  toggleBookmark: (
    sessionId: string,
    currentBookmarked?: boolean,
  ) => Promise<void>;

  /**
   * Update the user-visible title on a session.
   */
  renameSession: (sessionId: string, name: string) => Promise<void>;

  /**
   * Persist that the user viewed a session and clear unread state locally.
   */
  markSessionViewed: (sessionId: string, viewedAt?: Date) => Promise<void>;

  /**
   * Remove one pending approval from the session after a response.
   */
  clearPendingApproval: (sessionId: string, toolCallId: string) => void;
}
