import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAssistantContext } from '@/context/AssistantContext';
import {
  useAgentSessionListState,
  useAgentSessionListActions,
} from '@/context/AgentSessionListContext';
import { SessionCard } from './components/SessionCard';
import { AssistantSelectionCard } from './components/AssistantSelectionCard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';
import { RefreshCw, Search } from 'lucide-react';
import type { Assistant } from '@/models/chat';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { filterSessions } from '@/lib/session-utils';
import { getPlaybook } from '@/lib/backend/playbooks';

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
  const { sessions, isSessionsListLoading } = useAgentSessionListState();
  const { createSession, loadSessions, deleteSession } =
    useAgentSessionListActions();
  const [isCreating, setIsCreating] = useState(false);
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );
  const [searchParams] = useSearchParams();
  const [activeTab, setActiveTab] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');
  const processingPlaybookRef = useRef(false);

  // Handle Playbook Auto-Start
  useEffect(() => {
    const playbookId = searchParams.get('playbookId');
    if (playbookId && !processingPlaybookRef.current && assistants.length > 0) {
      // Temporary resources
      let toastId: string | number | undefined;

      const initPlaybookSession = async () => {
        try {
          processingPlaybookRef.current = true;
          logger.info('Auto-starting playbook session', { playbookId });

          if (!toastId) toastId = toast.loading('Starting playbook...');

          // Find assistant from playbookId by fetching all playbooks
          // We need to determine which assistant this playbook belongs to
          const allAssistants = assistants;
          let playbook = null;
          let targetAssistant = null;

          for (const assistant of allAssistants) {
            if (!assistant.id) {
              continue;
            }

            try {
              playbook = await getPlaybook(playbookId, assistant.id);
              if (playbook) {
                targetAssistant = assistant;
                break;
              }
            } catch {
              // Continue searching even if a lookup fails
              continue;
            }
          }

          if (!playbook || !targetAssistant) {
            if (toastId) toast.dismiss(toastId);
            toast.error('Playbook not found');
            return;
          }

          if (toastId)
            toast.loading(`Starting playbook: ${playbook.goal}`, {
              id: toastId,
            });

          // For now we use a simple loading toast during playbook session creation.
          // Correlating backend initialization events to the new session would require
          // additional refactoring of the session creation flow, so detailed progress
          // is intentionally deferred to keep this UI logic straightforward.

          const session = await createSession({
            assistant: targetAssistant,
            name: playbook.goal,
          });
          if (toastId) toast.dismiss(toastId);

          // Navigate to the new session with the playbookId param so AgentChatView can pick it up
          navigate(`/agent/${session.id}?playbookId=${playbookId}`);
        } catch (error) {
          if (toastId) toast.dismiss(toastId);
          logger.error('Failed to start playbook session', error);
          toast.error('Failed to start playbook session');
        } finally {
          processingPlaybookRef.current = false;
        }
      };

      initPlaybookSession();

      // Cleanup not needed as unlisten isn't used if we don't set it up
    }
  }, [searchParams, assistants, createSession, navigate]);

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

    let filtered = sessions;

    // Filter by Tab
    if (activeTab !== 'all') {
      filtered = filtered.filter((session) => session.status === activeTab);
    }

    // Filter by search query
    filtered = filterSessions(filtered, searchQuery);

    // Sort by status first, then by creation date
    return [...filtered].sort((a, b) => {
      const statusDiff = statusPriority[a.status] - statusPriority[b.status];
      if (statusDiff !== 0) return statusDiff;
      return b.createdAt.getTime() - a.createdAt.getTime();
    });
  }, [sessions, searchQuery, activeTab]);

  // Compute counts for tabs
  const statusCounts = useMemo(() => {
    const counts = {
      all: sessions.length,
      busy: 0,
      idle: 0,
      paused: 0,
      error: 0,
    };
    sessions.forEach((s) => {
      if (Object.prototype.hasOwnProperty.call(counts, s.status)) {
        counts[s.status as keyof typeof counts]++;
      }
    });
    return counts;
  }, [sessions]);

  const handleAssistantSelect = useCallback(
    async (assistant: Assistant) => {
      // Show loading state
      setStartingAssistantId(assistant.id || null);

      // Navigate to simplified draft view
      navigate(`/agent/draft?assistantId=${assistant.id}`);
    },
    [navigate],
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
    <main className="h-full w-full grid grid-cols-1 md:grid-cols-12 font-mono divide-y md:divide-y-0 md:divide-x">
      {/* Left Column - Assistant Selection */}
      <div
        className="md:col-span-5 lg:col-span-4 flex flex-col h-full overflow-hidden"
        role="region"
        aria-label="Assistant Selection"
      >
        <div className="p-6 border-b shrink-0">
          <h2 className="text-xl font-bold" id="assistant-heading">
            Select Assistant
          </h2>
          <p className="text-sm text-muted-foreground mt-1">
            Choose an assistant to start a new agent session
          </p>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <ul
            className="flex flex-col gap-4 max-w-2xl mx-auto list-none"
            aria-labelledby="assistant-heading"
          >
            {assistants.map((assistant) => {
              const isThisStarting = startingAssistantId === assistant.id;
              return (
                <li key={assistant.id}>
                  <AssistantSelectionCard
                    assistant={assistant}
                    isStarting={isThisStarting}
                    disabled={isCreating}
                    onSelect={(a) => {
                      setIsCreating(true);
                      handleAssistantSelect(a);
                    }}
                  />
                </li>
              );
            })}
          </ul>

          {assistants.length === 0 && (
            <div className="text-center text-muted-foreground py-12">
              <p>No assistants available.</p>
              <Link to="/assistants">
                <Button className="mt-4">Create Assistant</Button>
              </Link>
            </div>
          )}
        </div>

        <div className="p-6 border-t shrink-0">
          <Link to="/assistants">
            <Button variant="outline" disabled={isCreating} className="w-full">
              Manage Assistants
            </Button>
          </Link>
        </div>
      </div>

      {/* Right Column - Session History */}
      <div
        className="md:col-span-7 lg:col-span-8 flex flex-col h-full overflow-hidden"
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
              disabled={isSessionsListLoading}
              aria-label="Refresh sessions"
            >
              <RefreshCw
                className={cn(
                  'h-4 w-4',
                  isSessionsListLoading && 'animate-spin',
                )}
              />
            </Button>
          </div>

          <Tabs
            defaultValue="all"
            value={activeTab}
            onValueChange={setActiveTab}
            className="w-full mb-4"
          >
            <TabsList className="w-full justify-start overflow-x-auto">
              <TabsTrigger value="all" className="flex-1">
                All ({statusCounts.all})
              </TabsTrigger>
              <TabsTrigger value="busy" className="flex-1">
                Busy ({statusCounts.busy})
              </TabsTrigger>
              <TabsTrigger value="idle" className="flex-1">
                Idle ({statusCounts.idle})
              </TabsTrigger>
              <TabsTrigger value="paused" className="flex-1">
                Paused ({statusCounts.paused})
              </TabsTrigger>
              <TabsTrigger value="error" className="flex-1">
                Error ({statusCounts.error})
              </TabsTrigger>
            </TabsList>
          </Tabs>

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
          {isSessionsListLoading && sessions.length === 0 ? (
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
            <ul
              className="grid grid-cols-1 gap-4 max-w-2xl list-none"
              aria-labelledby="session-heading"
            >
              {filteredAndSortedSessions.map((session) => (
                <li key={session.id}>
                  <SessionCard
                    session={session}
                    onResume={handleResumeSession}
                    onDelete={handleDeleteSession}
                  />
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </main>
  );
}
