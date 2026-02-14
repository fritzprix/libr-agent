import { useState, useCallback, useEffect, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { useAssistantContext } from '@/context/AssistantContext';
import {
  useAgentSessionListState,
  useAgentSessionListActions,
} from '@/context/AgentSessionListContext';
import { AssistantSelectionCard } from './components/AssistantSelectionCard';
import { SessionHistoryPanel } from './components/SessionHistoryPanel';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import type { Assistant } from '@/models/chat';
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

  const handleAssistantSelect = useCallback(
    async (assistant: Assistant) => {
      // Show loading state
      setStartingAssistantId(assistant.id);

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
        <SessionHistoryPanel
          sessions={sessions}
          isLoading={isSessionsListLoading}
          activeTab={activeTab}
          searchQuery={searchQuery}
          onActiveTabChange={setActiveTab}
          onSearchQueryChange={setSearchQuery}
          onRefresh={handleRefreshSessions}
          onResume={handleResumeSession}
          onDelete={handleDeleteSession}
        />
      </div>
    </main>
  );
}
