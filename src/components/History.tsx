import { useCallback } from "react";
import { useSessionContext } from "../context/SessionContext";
import SessionList from "./SessionList";
import { Card, CardContent, CardHeader, CardTitle } from "./ui";

export default function History() {
  const {
    sessions: sessionPages,
    current: currentSession,
    select,
    delete: deleteSession,
    loadMore,
    isLoading,
  } = useSessionContext();

  // Flatten the paginated sessions
  const sessions = sessionPages ? sessionPages.flatMap((p) => p.items) : [];

  const handleSelectSession = useCallback(
    (sessionId: string) => {
      select(sessionId);
    },
    [select],
  );

  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      await deleteSession(sessionId);
    },
    [deleteSession],
  );

  const handleLoadMore = useCallback(() => {
    loadMore();
  }, [loadMore]);

  return (
    <div className="flex-1 flex flex-col p-6 bg-gray-900 text-white">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-green-400 mb-2">
          Session History
        </h1>
        <p className="text-gray-400">
          Browse and manage your conversation sessions
        </p>
      </div>

      <Card className="flex-1 bg-gray-800 border-gray-700">
        <CardHeader>
          <CardTitle className="text-lg text-green-400">
            All Sessions ({sessions.length})
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex flex-col">
          {isLoading && sessions.length === 0 ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-gray-500">Loading sessions...</div>
            </div>
          ) : (
            <>
              <SessionList
                sessions={sessions}
                currentSessionId={currentSession?.id}
                onSelectSession={handleSelectSession}
                onDeleteSession={handleDeleteSession}
                showSearch={true}
                className="flex-1"
                emptyMessage="No sessions yet. Start a conversation to create your first session."
              />

              {/* Load more button if there are more pages */}
              {sessionPages &&
                sessionPages.length > 0 &&
                sessionPages[sessionPages.length - 1]?.hasNextPage && (
                  <div className="mt-4 text-center">
                    <button
                      onClick={handleLoadMore}
                      disabled={isLoading}
                      className="px-4 py-2 bg-green-900/20 border border-green-400 text-green-400 rounded hover:bg-green-900/40 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {isLoading ? "Loading..." : "Load More"}
                    </button>
                  </div>
                )}
            </>
          )}
        </CardContent>
      </Card>

      {currentSession && (
        <div className="mt-4">
          <Card className="bg-gray-800 border-gray-700">
            <CardHeader>
              <CardTitle className="text-md text-green-400">
                Selected Session
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                <div>
                  <span className="text-gray-400">Name: </span>
                  <span className="text-white">{currentSession.name}</span>
                </div>
                <div>
                  <span className="text-gray-400">Type: </span>
                  <span className="text-white capitalize">
                    {currentSession.type}
                  </span>
                </div>
                <div>
                  <span className="text-gray-400">Assistants: </span>
                  <span className="text-white">
                    {currentSession.assistants.map((a) => a.name).join(", ")}
                  </span>
                </div>
                <div>
                  <span className="text-gray-400">Created: </span>
                  <span className="text-white">
                    {new Date(currentSession.createdAt).toLocaleString()}
                  </span>
                </div>
                {currentSession.description && (
                  <div>
                    <span className="text-gray-400">Description: </span>
                    <span className="text-white">
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
