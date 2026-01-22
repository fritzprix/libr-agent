import { useState, useMemo } from 'react';
import useSWR from 'swr';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
} from '../../components/ui';

import { useAgentSessionListState } from '@/context/AgentSessionListContext';
import SessionList from '../session/SessionList';
import { getLogger } from '@/lib/logger';
import { searchMessages } from '@/lib/rust-backend-client';
import { useDebounced } from '@/hooks/useDebounced';
import type { SortMode } from '@/models/search';
import { Search, ArrowUp, ArrowDown } from 'lucide-react';

const logger = getLogger('History');

export default function History() {
  const { sessions, isSessionsListLoading: isLoading } =
    useAgentSessionListState();

  const [query, setQuery] = useState('');
  const [sortMode, setSortMode] = useState<SortMode>('desc');
  const debouncedQuery = useDebounced(query, 300);
  const pageSize = 200;

  // sessions is directly available from context as array
  // No need to flatMap sessionPages

  // Build SWR key only when query is non-empty
  const swrKey = debouncedQuery?.trim()
    ? ['historySearch', debouncedQuery.trim(), sortMode, pageSize]
    : null;

  const {
    data: searchPage,
    error,
    isValidating,
  } = useSWR(
    swrKey,
    async () => {
      logger.debug('Fetching search results', {
        query: debouncedQuery,
        pageSize,
      });
      // Call searchMessages without sessionId for global search
      return await searchMessages(
        debouncedQuery!.trim(),
        undefined,
        1,
        pageSize,
      );
    },
    {
      revalidateOnFocus: false,
      dedupingInterval: 5000,
      onError: (err: Error) => {
        logger.error('Search failed', err);
      },
    },
  );

  const searchHitsMap = useMemo(() => {
    const m = new Map<string, number>();

    // accumulate search hits per session (if any)
    if (searchPage?.items && searchPage.items.length > 0) {
      for (const item of searchPage.items) {
        const current = m.get(item.sessionId) || 0;
        m.set(item.sessionId, current + 1);
      }
    }

    return m;
  }, [searchPage]);

  const orderedSessions = useMemo(() => {
    const arr = [...sessions];

    // If a search query is active, surface only sessions that have hits
    // and order them by hits desc, then newest first as tiebreaker.
    if (debouncedQuery?.trim()) {
      return arr
        .filter((s) => (searchHitsMap.get(s.id) ?? 0) > 0)
        .sort((a, b) => {
          const hitsA = searchHitsMap.get(a.id) ?? 0;
          const hitsB = searchHitsMap.get(b.id) ?? 0;
          const hitDiff = hitsB - hitsA;
          if (hitDiff !== 0) return hitDiff;
          return (
            new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
          );
        });
    }

    // No search: order by createdAt using the selected sortMode
    return arr.sort((a, b) => {
      const at = new Date(a.createdAt).getTime();
      const bt = new Date(b.createdAt).getTime();
      return sortMode === 'asc' ? at - bt : bt - at;
    });
  }, [sessions, searchHitsMap, debouncedQuery, sortMode]);

  // Pagination handling removed for Agent V2 initial migration (full list load)

  const searchState = useMemo(
    () => ({
      isSearching: isValidating,
      hasResults: !!searchPage?.items.length,
      error: error ?? null,
    }),
    [isValidating, searchPage?.items, error],
  );

  return (
    <div className="flex-1 flex flex-col p-6 text-foreground">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-primary mb-2">
          Session History
        </h1>
        <p className="text-muted-foreground">
          Browse and manage your conversation sessions
        </p>
      </div>

      <Card className="flex-1 border-muted">
        <CardHeader>
          <CardTitle className="text-lg text-primary">
            All Sessions ({orderedSessions.length})
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex flex-col">
          {/* Search Header */}
          <div className="mb-4 space-y-3">
            {/* Search Input */}
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <Input
                type="text"
                placeholder="Search messages across all sessions..."
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="pl-9"
              />
            </div>

            {/* Sort Toggle (only visible when no search query) */}
            {!debouncedQuery?.trim() && (
              <div className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground">
                  Sort by date:
                </span>
                <div className="flex gap-1">
                  <Button
                    variant={sortMode === 'asc' ? 'default' : 'outline'}
                    size="sm"
                    onClick={() => setSortMode('asc')}
                    className="flex items-center gap-1"
                  >
                    <ArrowUp className="h-3 w-3" />
                    <span>Oldest first</span>
                  </Button>
                  <Button
                    variant={sortMode === 'desc' ? 'default' : 'outline'}
                    size="sm"
                    onClick={() => setSortMode('desc')}
                    className="flex items-center gap-1"
                  >
                    <ArrowDown className="h-3 w-3" />
                    <span>Newest first</span>
                  </Button>
                </div>
              </div>
            )}

            {/* Search Status */}
            {searchState.isSearching && (
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <div className="animate-spin h-4 w-4 border-2 border-current border-t-transparent rounded-full" />
                <span>Searching...</span>
              </div>
            )}

            {searchState.error && (
              <div className="text-sm text-destructive">
                Search failed. Please try again.
              </div>
            )}

            {debouncedQuery?.trim() &&
              !searchState.isSearching &&
              !searchState.hasResults && (
                <div className="text-sm text-muted-foreground">
                  No results found for &ldquo;{debouncedQuery}&rdquo;
                </div>
              )}
          </div>

          {/* Session List */}
          {isLoading && sessions.length === 0 ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-muted-foreground">Loading sessions...</div>
            </div>
          ) : (
            <>
              <SessionList
                sessions={orderedSessions}
                searchHits={searchHitsMap}
                showSearch={false}
                className="flex-1"
                emptyMessage="No sessions yet. Start a conversation to create your first session."
                isCollapsed={false}
              />

              {/* Pagination is handled by virtual scroll or full load in Agent V2 currently */}
            </>
          )}
        </CardContent>
      </Card>

      {/* Selected Session detail view removed - Navigation is direct to session */}
    </div>
  );
}
