import {
  Search as SearchIcon,
  ChevronLeft,
  ChevronRight,
  AlertCircle,
} from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Badge,
} from '@/components/ui';
import { searchMessages, MessageSearchResult } from '@/lib/rust-backend-client';
import { useSessionContext } from '@/context/SessionContext';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MessageSearch');

interface SearchState {
  query: string;
  sessionId: string;
  page: number;
  pageSize: number;
  isLoading: boolean;
  error: string | null;
  results: MessageSearchResult[];
  totalItems: number;
  hasNextPage: boolean;
  hasPreviousPage: boolean;
}

export default function MessageSearch() {
  const { sessions: sessionPages } = useSessionContext();

  // Flatten paginated sessions
  const sessions = useMemo(
    () => (sessionPages ? sessionPages.flatMap((p) => p.items) : []),
    [sessionPages],
  );

  const [state, setState] = useState<SearchState>({
    query: '',
    sessionId: '',
    page: 1,
    pageSize: 25,
    isLoading: false,
    error: null,
    results: [],
    totalItems: 0,
    hasNextPage: false,
    hasPreviousPage: false,
  });

  const [inputQuery, setInputQuery] = useState('');
  const [selectedSessionId, setSelectedSessionId] = useState('');

  // Execute search
  const handleSearch = useCallback(
    async (
      query: string,
      sessionId: string,
      page: number,
      pageSize: number,
    ) => {
      if (!query.trim()) {
        setState((prev) => ({
          ...prev,
          error: 'Please enter a search query',
          results: [],
        }));
        return;
      }

      if (!sessionId) {
        setState((prev) => ({
          ...prev,
          error: 'Please select a session to search',
          results: [],
        }));
        return;
      }

      setState((prev) => ({ ...prev, isLoading: true, error: null }));

      try {
        logger.info('Searching messages', { query, sessionId, page, pageSize });

        const result = await searchMessages(query, sessionId, page, pageSize);

        logger.info('Search results received', {
          totalItems: result.totalItems,
          resultsCount: result.items.length,
        });

        setState({
          query,
          sessionId,
          page,
          pageSize,
          isLoading: false,
          error: null,
          results: result.items,
          totalItems: result.totalItems,
          hasNextPage: result.hasNextPage,
          hasPreviousPage: result.hasPreviousPage,
        });
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Failed to search messages';
        logger.error('Search failed', error);
        setState((prev) => ({
          ...prev,
          isLoading: false,
          error: errorMessage,
          results: [],
        }));
      }
    },
    [],
  );

  // Handle search button click
  const handleSearchClick = useCallback(() => {
    handleSearch(inputQuery, selectedSessionId, 1, state.pageSize);
  }, [inputQuery, selectedSessionId, state.pageSize, handleSearch]);

  // Handle page navigation
  const handlePageChange = useCallback(
    (newPage: number) => {
      if (state.query && state.sessionId) {
        handleSearch(state.query, state.sessionId, newPage, state.pageSize);
      }
    },
    [state.query, state.sessionId, state.pageSize, handleSearch],
  );

  // Handle retry on error
  const handleRetry = useCallback(() => {
    if (state.query && state.sessionId) {
      handleSearch(state.query, state.sessionId, state.page, state.pageSize);
    }
  }, [state.query, state.sessionId, state.page, state.pageSize, handleSearch]);

  // Handle Enter key in search input
  const handleKeyPress = useCallback(
    (e: { key: string }) => {
      if (e.key === 'Enter') {
        handleSearchClick();
      }
    },
    [handleSearchClick],
  );

  // Calculate total pages
  const totalPages = useMemo(() => {
    return Math.ceil(state.totalItems / state.pageSize);
  }, [state.totalItems, state.pageSize]);

  // Format date
  const formatDate = (date: Date) => {
    // Check if date is valid
    if (!date || isNaN(date.getTime())) {
      return 'Invalid Date';
    }
    try {
      return new Intl.DateTimeFormat('ko-KR', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      }).format(date);
    } catch {
      return 'Invalid Date';
    }
  };

  // Get session name by ID
  const getSessionName = useCallback(
    (sessionId: string) => {
      const session = sessions.find((s) => s.id === sessionId);
      return session?.name || 'Unknown Session';
    },
    [sessions],
  );

  return (
    <div className="flex-1 flex flex-col p-6 text-foreground">
      {/* Header */}
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-primary mb-2">
          Message Search (BM25)
        </h1>
        <p className="text-muted-foreground">
          Search messages using full-text BM25 algorithm with relevance scoring
        </p>
      </div>

      {/* Search Controls */}
      <Card className="mb-6 border-muted">
        <CardHeader>
          <CardTitle className="text-lg text-primary">Search Query</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col gap-4">
            {/* Search Input */}
            <div className="flex gap-2">
              <Input
                placeholder="Enter search query..."
                value={inputQuery}
                onChange={(e) => setInputQuery(e.target.value)}
                onKeyPress={handleKeyPress}
                className="flex-1"
                disabled={state.isLoading}
              />
              <Button
                onClick={handleSearchClick}
                disabled={
                  state.isLoading || !inputQuery.trim() || !selectedSessionId
                }
                className="min-w-[100px]"
              >
                <SearchIcon size={16} className="mr-2" />
                Search
              </Button>
            </div>

            {/* Session Selector */}
            <div className="flex flex-col gap-2">
              <label className="text-sm font-medium text-muted-foreground">
                Select Session
              </label>
              <select
                value={selectedSessionId}
                onChange={(e) => setSelectedSessionId(e.target.value)}
                disabled={state.isLoading}
                className="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground focus:outline-none focus:ring-2 focus:ring-primary"
              >
                <option value="">-- Select a session --</option>
                {sessions.map((session) => (
                  <option key={session.id} value={session.id}>
                    {session.name} (
                    {new Date(session.createdAt).toLocaleDateString()})
                  </option>
                ))}
              </select>
            </div>

            {/* Search Stats */}
            {state.query && !state.error && (
              <div className="text-sm text-muted-foreground">
                Query:{' '}
                <span className="font-medium text-foreground">
                  {state.query}
                </span>
                {' | '}
                Session:{' '}
                <span className="font-medium text-foreground">
                  {getSessionName(state.sessionId)}
                </span>
                {' | '}
                Results:{' '}
                <span className="font-medium text-foreground">
                  {state.totalItems}
                </span>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Error Display */}
      {state.error && (
        <Card className="mb-6 border-destructive bg-destructive/10">
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <AlertCircle size={20} className="text-destructive" />
              <div className="flex-1">
                <p className="text-destructive font-medium">{state.error}</p>
              </div>
              {state.query && state.sessionId && (
                <Button
                  onClick={handleRetry}
                  variant="outline"
                  size="sm"
                  disabled={state.isLoading}
                >
                  Retry
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Loading State */}
      {state.isLoading && (
        <Card className="flex-1 border-muted">
          <CardContent className="pt-6 flex items-center justify-center h-full">
            <div className="text-center">
              <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary mx-auto mb-4"></div>
              <p className="text-muted-foreground">Searching messages...</p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Results Display */}
      {!state.isLoading && state.results.length > 0 && (
        <Card className="flex-1 border-muted">
          <CardHeader>
            <CardTitle className="text-lg text-primary">
              Search Results ({state.totalItems} found)
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {state.results.map((result) => (
              <div
                key={result.messageId}
                className="p-4 border border-muted rounded-lg hover:bg-muted/50 transition-colors"
              >
                {/* Header with Score and Session */}
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary" className="font-mono">
                      Score: {result.score.toFixed(2)}
                    </Badge>
                    <span className="text-sm text-muted-foreground">
                      {getSessionName(result.sessionId)}
                    </span>
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {formatDate(result.createdAt)}
                  </span>
                </div>

                {/* Snippet */}
                <div className="text-sm text-foreground leading-relaxed">
                  {result.snippet || 'No snippet available'}
                </div>

                {/* Message ID */}
                <div className="mt-2 text-xs text-muted-foreground font-mono">
                  ID: {result.messageId}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {/* Empty State */}
      {!state.isLoading &&
        state.query &&
        state.results.length === 0 &&
        !state.error && (
          <Card className="flex-1 border-muted">
            <CardContent className="pt-6 flex items-center justify-center h-full">
              <div className="text-center">
                <SearchIcon
                  size={48}
                  className="mx-auto mb-4 text-muted-foreground"
                />
                <p className="text-muted-foreground text-lg mb-2">
                  No results found
                </p>
                <p className="text-sm text-muted-foreground">
                  Try a different search query or session
                </p>
              </div>
            </CardContent>
          </Card>
        )}

      {/* Initial State */}
      {!state.isLoading && !state.query && !state.error && (
        <Card className="flex-1 border-muted">
          <CardContent className="pt-6 flex items-center justify-center h-full">
            <div className="text-center">
              <SearchIcon
                size={48}
                className="mx-auto mb-4 text-muted-foreground"
              />
              <p className="text-muted-foreground text-lg mb-2">
                Start searching messages
              </p>
              <p className="text-sm text-muted-foreground">
                Enter a query and select a session to begin
              </p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Pagination */}
      {!state.isLoading && state.results.length > 0 && totalPages > 1 && (
        <div className="mt-6 flex items-center justify-between">
          <Button
            onClick={() => handlePageChange(state.page - 1)}
            disabled={!state.hasPreviousPage || state.isLoading}
            variant="outline"
          >
            <ChevronLeft size={16} className="mr-1" />
            Previous
          </Button>

          <span className="text-sm text-muted-foreground">
            Page {state.page} of {totalPages}
          </span>

          <Button
            onClick={() => handlePageChange(state.page + 1)}
            disabled={!state.hasNextPage || state.isLoading}
            variant="outline"
          >
            Next
            <ChevronRight size={16} className="ml-1" />
          </Button>
        </div>
      )}
    </div>
  );
}
