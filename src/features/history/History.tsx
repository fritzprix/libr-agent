import { useCallback, useMemo } from 'react';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '../../components/ui';
import { useSessionContext } from '@/context/SessionContext';
import SessionList from '../session/SessionList';

export default function History() {
  const {
    sessions: sessionPages,
    current: currentSession,
    loadMore,
    isLoading,
    hasNextPage,
  } = useSessionContext();

  // Flatten the paginated sessions
  const sessions = useMemo(
    () => (sessionPages ? sessionPages.flatMap((p) => p.items) : []),
    [sessionPages],
  );

  const handleLoadMore = useCallback(() => {
    loadMore();
  }, [loadMore]);

  return (
    <div className="flex-1 flex flex-col p-6 bg-muted text-foreground">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-primary mb-2">
          Session History
        </h1>
        <p className="text-muted-foreground">
          Browse and manage your conversation sessions
        </p>
      </div>

      <Card className="flex-1 bg-muted border-muted">
        <CardHeader>
          <CardTitle className="text-lg text-primary">
            All Sessions ({sessions.length})
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex flex-col">
          {isLoading && sessions.length === 0 ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-muted-foreground">Loading sessions...</div>
            </div>
          ) : (
            <>
              <SessionList
                sessions={sessions}
                showSearch={true}
                className="flex-1"
                emptyMessage="No sessions yet. Start a conversation to create your first session."
              />

              {/* Load more button if there are more pages */}
              {hasNextPage && (
                <div className="mt-4 text-center">
                  <Button
                    onClick={handleLoadMore}
                    disabled={isLoading}
                    className="px-4 py-2 bg-primary/10 border border-primary text-primary rounded hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed"
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
