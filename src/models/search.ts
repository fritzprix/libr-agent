import type { Session } from './chat';

/**
 * Aggregated search result for a single session
 */
export interface SearchAggregation {
  sessionId: string;
  count: number;
  latest: number;
  snippet?: string;
}

/**
 * Session extended with search hit count
 */
export interface SessionWithHits extends Session {
  searchHits?: number;
}

/**
 * Sort mode for session list
 */
export type SortMode = 'asc' | 'desc';

/**
 * Search state for UI rendering
 */
export interface SearchState {
  isSearching: boolean;
  hasResults: boolean;
  error: Error | null;
}
