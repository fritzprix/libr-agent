import { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { SessionHistoryPanel } from '@/features/agent/components/SessionHistoryPanel';

const logger = getLogger('History');

export default function History() {
  const navigate = useNavigate();
  const { sessions, isSessionsListLoading } = useAgentSessionListState();
  const { loadSessions, deleteSession, deleteSessionOnly } =
    useAgentSessionListActions();
  const [activeTab, setActiveTab] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

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
      } catch (error) {
        logger.error('Failed to delete session', error);
        toast.error('Failed to delete session');
      }
    },
    [deleteSession],
  );

  const handleDeleteSessionOnly = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSessionOnly(sessionId);
        toast.success('Session deleted');
      } catch (error) {
        logger.error('Failed to delete session only', error);
        toast.error('Failed to delete session');
      }
    },
    [deleteSessionOnly],
  );

  const handleRefreshSessions = useCallback(() => {
    loadSessions();
  }, [loadSessions]);

  return (
    <div className="flex-1 flex flex-col text-foreground overflow-hidden">
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
        onDeleteOnly={handleDeleteSessionOnly}
        heading="Session History"
        description="Browse and manage your conversation sessions"
        emptyStateTitle="No sessions yet"
        emptyStateSubtitle="Start a conversation to create your first session"
      />
    </div>
  );
}
