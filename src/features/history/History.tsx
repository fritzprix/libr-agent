import { useState, useCallback, useMemo } from 'react';
import useSWR from 'swr';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
} from '../../components/ui';
import { useSessionContext } from '@/context/SessionContext';
import SessionList from '../session/SessionList';
import { getLogger } from '@/lib/logger';
import { searchMessages } from '@/lib/rust-backend-client';
import { useDebounced } from '@/hooks/useDebounced';
import type { SessionWithHits, SortMode } from '@/models/search';
import { Search, ArrowUp, ArrowDown } from 'lucide-react';

const logger = getLogger('History');

export default function History() {
  const {
    sessions: sessionPages,
    current: currentSession,
    loadMore,
    isLoading,
    hasNextPage,
  } = useSessionContext();

  const [query, setQuery] = useState('');
  const [sortMode, setSortMode] = useState<SortMode>('desc');
  const debouncedQuery = useDebounced(query, 300);
  const pageSize = 200;

  // Flatten the paginated sessions
  const sessions = useMemo(
    () => (sessionPages ? sessionPages.flatMap((p) => p.items) : []),
    [sessionPages],
  );

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

  const sessionsWithHits = useMemo(() => {
    const m = new Map<string, SessionWithHits>();
    // initialize all sessions with zero hits
    for (const sess of sessions) {
      m.set(sess.id, { ...sess, searchHits: 0 });
    }

    // accumulate search hits per session (if any)
    if (searchPage?.items && searchPage.items.length > 0) {
      for (const item of searchPage.items) {
        const sess = m.get(item.sessionId);
        if (sess) {
          m.set(item.sessionId, {
            ...sess,
            searchHits: (sess.searchHits ?? 0) + 1,
          });
        }
      }
    }

    return m;
  }, [searchPage, sessions]);

  const orderedSessions = useMemo(() => {
    const arr = [...sessionsWithHits.values()];

    // If a search query is active, surface only sessions that have hits
    // and order them by hits desc, then newest first as tiebreaker.
    if (debouncedQuery?.trim()) {
      return arr
        .filter((s) => (s.searchHits ?? 0) > 0)
        .sort((a, b) => {
          const hitDiff = (b.searchHits ?? 0) - (a.searchHits ?? 0);
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
  }, [sessionsWithHits, debouncedQuery, sortMode]);

  const handleLoadMore = useCallback(() => {
    loadMore();
  }, [loadMore]);

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
                showSearch={false}
                className="flex-1"
                emptyMessage="No sessions yet. Start a conversation to create your first session."
                isCollapsed={false}
              />

              {/* Load more button if there are more pages and no search active */}
              {hasNextPage && !debouncedQuery?.trim() && (
                <div className="mt-4 text-center">
                  <Button
                    onClick={handleLoadMore}
                    disabled={isLoading}
                    variant="outline"
                    className="text-primary border-primary hover:bg-primary/10 px-4 py-2 rounded disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {isLoading ? 'Loading...' : 'Load More'}
                  </Button>
                </div>
              )}
            </>
          )}
        </CardContent>
      </Card>

      {currentSession && (
        <div className="mt-4">
          <Card className="bg-muted border-muted">
            <CardHeader>
              <CardTitle className="text-md text-primary">
                Selected Session
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                <div>
                  <span className="text-muted-foreground">Name: </span>
                  <span className="text-foreground">{currentSession.name}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">Type: </span>
                  <span className="text-foreground capitalize">
                    {currentSession.type}
                  </span>
                </div>
                <div>
                  <span className="text-muted-foreground">Assistants: </span>
                  <span className="text-foreground">
                    {currentSession.assistants.map((a) => a.name).join(', ')}
                  </span>
                </div>
                <div>
                  <span className="text-muted-foreground">Created: </span>
                  <span className="text-foreground">
                    {new Date(currentSession.createdAt).toLocaleString()}
                  </span>
                </div>
                {currentSession.description && (
                  <div>
                    <span className="text-muted-foreground">Description: </span>
                    <span className="text-foreground">
                      {currentSession.description}
                    </span>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
