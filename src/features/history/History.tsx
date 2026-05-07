import { useCallback, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { SessionHistoryPanel } from '@/features/agent/components/SessionHistoryPanel';
import type { SessionStatus } from '@/lib/session-utils';

const logger = getLogger('History');

export default function History() {
  const navigate = useNavigate();
  const { t } = useTranslation('common');
  const {
    sessions,
    isSessionsListLoading,
    hasMoreSessions,
    isLoadingMoreSessions,
  } = useAgentSessionListState();
  const {
    loadSessions,
    loadMoreSessions,
    deleteSession,
    deleteSessionOnly,
    toggleBookmark,
  } = useAgentSessionListActions();
  const [activeTab, setActiveTab] = useState('all');
  const [activeStatusFilter, setActiveStatusFilter] = useState<
    'all' | SessionStatus
  >('all');
  const [searchQuery, setSearchQuery] = useState('');

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
        toast.success(t('sessionHistory.toasts.deleted', 'Session deleted'));
      } catch (error) {
        logger.error('Failed to delete session', error);
        toast.error(
          t('sessionHistory.toasts.deleteFailed', 'Failed to delete session'),
        );
      }
    },
    [deleteSession, t],
  );

  const handleDeleteSessionOnly = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSessionOnly(sessionId);
        toast.success(t('sessionHistory.toasts.deleted', 'Session deleted'));
      } catch (error) {
        logger.error('Failed to delete session only', error);
        toast.error(
          t('sessionHistory.toasts.deleteFailed', 'Failed to delete session'),
        );
      }
    },
    [deleteSessionOnly, t],
  );

  const handleRefreshSessions = useCallback(() => {
    loadSessions(true);
  }, [loadSessions]);

  const handleLoadMoreSessions = useCallback(() => {
    void loadMoreSessions().catch((error) => {
      logger.error('Failed to load more sessions', error);
      toast.error(
        t(
          'sessionHistory.toasts.loadMoreFailed',
          'Failed to load more sessions',
        ),
      );
    });
  }, [loadMoreSessions, t]);

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden text-foreground">
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={isSessionsListLoading}
        hasMoreSessions={hasMoreSessions}
        isLoadingMoreSessions={isLoadingMoreSessions}
        activeTab={activeTab}
        activeStatusFilter={activeStatusFilter}
        searchQuery={searchQuery}
        onActiveTabChange={setActiveTab}
        onActiveStatusFilterChange={setActiveStatusFilter}
        onSearchQueryChange={setSearchQuery}
        onRefresh={handleRefreshSessions}
        onLoadMore={handleLoadMoreSessions}
        onResume={handleResumeSession}
        onDelete={handleDeleteSession}
        onDeleteOnly={handleDeleteSessionOnly}
        onToggleBookmark={toggleBookmark}
        heading={t('sessionHistory.heading', 'Session History')}
        description={t(
          'sessionHistory.description',
          'Browse and manage your conversation sessions',
        )}
        emptyStateTitle={t(
          'sessionHistory.emptyState.title',
          'No sessions yet',
        )}
        emptyStateSubtitle={t(
          'sessionHistory.emptyState.subtitle',
          'Start a conversation to create your first session',
        )}
      />
    </div>
  );
}
