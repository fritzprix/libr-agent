import { useState, useCallback, useEffect, useMemo } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAssistantContext } from '@/context/AssistantContext';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from '@/context/AgentSessionContext';
import { SessionCard } from './components/SessionCard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';
import { RefreshCw, Search } from 'lucide-react';
import type { Assistant } from '@/models/chat';

const logger = getLogger('AgentChatStartView');

/**
 * Agent Chat Start View - Two-Column Layout
 *
 * Enhanced start screen with assistant selection and session history.
 *
 * Layout:
 * - Left Column (40%): Assistant selection cards
 * - Right Column (60%): Session history (future implementation)
 *
 * Features:
 * - Click entire assistant card to select and create session
 * - "Starting..." pulse animation during session creation
 * - Disabled state during operations
 * - Responsive grid layout for assistant cards
 */
export default function AgentChatStartView() {
  const navigate = useNavigate();
  const { assistants } = useAssistantContext();
  const { isLoading, sessions, isLoadingSessions } = useAgentSessionState();
  const { createSession, loadSessions, deleteSession } =
    useAgentSessionActions();
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );
  const [searchQuery, setSearchQuery] = useState('');

  // Load sessions on mount
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Filter and sort sessions
  const filteredAndSortedSessions = useMemo(() => {
    const statusPriority = {
      busy: 1,
      idle: 2,
      paused: 3,
      error: 4,
    };

    // Filter by search query
    let filtered = sessions;
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = sessions.filter(
        (session) =>
          session.name?.toLowerCase().includes(query) ||
          session.id.toLowerCase().includes(query),
      );
    }

    // Sort by status first, then by creation date
    return [...filtered].sort((a, b) => {
      const statusDiff = statusPriority[a.status] - statusPriority[b.status];
      if (statusDiff !== 0) return statusDiff;
      return b.createdAt.getTime() - a.createdAt.getTime();
    });
  }, [sessions, searchQuery]);

  const handleAssistantSelect = useCallback(
    async (assistant: Assistant) => {
      if (isLoading) return; // Prevent duplicate clicks

      try {
        setStartingAssistantId(assistant.id || null);
        logger.info('Creating agent session with assistant', {
          assistantId: assistant.id,
          assistantName: assistant.name,
        });

        const session = await createSession({ assistant });
        logger.info('Agent session created successfully', {
          sessionId: session.id,
        });
        toast.success('Agent session started');

        // Reload sessions list
        await loadSessions();

        // Navigate to session-specific route
        navigate(`/agent/${session.id}`);
      } catch (err) {
        logger.error('Failed to create agent session', err);
        toast.error('Failed to start agent session');
      } finally {
        setStartingAssistantId(null);
      }
    },
    [createSession, navigate, isLoading, loadSessions],
  );

  const handleResumeSession = useCallback(
    (sessionId: string) => {
      navigate(`/agent/${sessionId}`);
    },
    [navigate],
  );

  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSession(sessionId);
        toast.success('Session deleted');
      } catch (err) {
        logger.error('Failed to delete session', err);
        toast.error('Failed to delete session');
      }
    },
    [deleteSession],
  );

  const handleRefreshSessions = useCallback(() => {
    loadSessions();
  }, [loadSessions]);

  return (
    <div className="h-full w-full flex font-mono" role="main">
      {/* Left Column - Assistant Selection */}
      <div
        className="flex-[2] border-r flex flex-col"
        role="region"
        aria-label="Assistant Selection"
      >
        <div className="p-6 border-b">
          <h2 className="text-xl font-bold" id="assistant-heading">
            Select Assistant
          </h2>
          <p className="text-sm text-muted-foreground mt-1">
            Choose an assistant to start a new agent session
          </p>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <div
            className="grid grid-cols-1 gap-4 max-w-2xl"
            role="list"
            aria-labelledby="assistant-heading"
          >
            {assistants.map((assistant) => {
              const isThisStarting = startingAssistantId === assistant.id;
              return (
                <button
                  key={assistant.id}
                  onClick={() => !isLoading && handleAssistantSelect(assistant)}
                  disabled={isLoading}
                  aria-label={`Start session with ${assistant.name}`}
                  aria-busy={isThisStarting}
                  aria-disabled={isLoading}
                  role="listitem"
                  className={cn(
                    'w-full p-4 text-left border rounded-lg transition-all',
                    'hover:shadow-md hover:scale-[1.01] focus:outline-none focus:ring-2 focus:ring-primary',
                    isLoading &&
                      !isThisStarting &&
                      'opacity-50 cursor-not-allowed',
                    isThisStarting &&
                      'border-primary bg-primary/10 animate-pulse',
                    !isLoading && 'hover:border-muted-foreground',
                  )}
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="font-semibold text-lg flex items-center gap-2">
                        {assistant.name}
                        {isThisStarting && (
                          <span className="text-sm text-primary font-normal">
                            Starting...
                          </span>
                        )}
                      </div>
                      {assistant.systemPrompt && (
                        <p className="text-sm text-muted-foreground mt-2 line-clamp-3">
                          {assistant.systemPrompt}
                        </p>
                      )}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>

          {assistants.length === 0 && (
            <div className="text-center text-muted-foreground py-12">
              <p>No assistants available.</p>
              <Link to="/assistants">
                <Button className="mt-4">Create Assistant</Button>
              </Link>
            </div>
          )}
        </div>

        <div className="p-6 border-t">
          <Link to="/assistants">
            <Button variant="outline" disabled={isLoading} className="w-full">
              Manage Assistants
            </Button>
          </Link>
        </div>
      </div>

      {/* Right Column - Session History */}
      <div
        className="flex-[3] flex flex-col"
        role="region"
        aria-label="Session History"
      >
        <div className="p-6 border-b">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h2 className="text-xl font-bold" id="session-heading">
                Recent Sessions
              </h2>
              <p className="text-sm text-muted-foreground mt-1">
                Resume previous agent sessions (
                {filteredAndSortedSessions.length}/{sessions.length})
              </p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleRefreshSessions}
              disabled={isLoadingSessions}
              aria-label="Refresh sessions"
            >
              <RefreshCw
                className={cn('h-4 w-4', isLoadingSessions && 'animate-spin')}
              />
            </Button>
          </div>

          {/* Search Input */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              type="text"
              placeholder="Search sessions by name or ID..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
              aria-label="Search sessions"
            />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          {isLoadingSessions && sessions.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-center text-muted-foreground">
                <RefreshCw className="h-8 w-8 animate-spin mx-auto mb-2" />
                <p className="text-sm">Loading sessions...</p>
              </div>
            </div>
          ) : filteredAndSortedSessions.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-center text-muted-foreground">
                {searchQuery.trim() ? (
                  <>
                    <p className="text-sm">
                      No sessions found matching &quot;{searchQuery}&quot;
                    </p>
                    <Button
                      variant="link"
                      size="sm"
                      onClick={() => setSearchQuery('')}
                      className="mt-2"
                    >
                      Clear search
                    </Button>
                  </>
                ) : (
                  <>
                    <p className="text-sm">No previous sessions</p>
                    <p className="text-xs mt-2">
                      Select an assistant to start your first session
                    </p>
                  </>
                )}
              </div>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4 max-w-2xl">
              {filteredAndSortedSessions.map((session) => (
                <SessionCard
                  key={session.id}
                  session={session}
                  onResume={handleResumeSession}
                  onDelete={handleDeleteSession}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
