import { useCallback, useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { SessionHistoryPanel } from '@/features/agent/components/SessionHistoryPanel';
import type { SessionStatus } from '@/lib/session-utils';
import { useSessionHistorySearch } from './useSessionHistorySearch';

const logger = getLogger('History');

export default function History() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useTranslation('common');
  const {
    sessions,
    isSessionsListLoading,
    hasMoreSessions,
    isLoadingMoreSessions,
    loadingChildrenParentIds,
  } = useAgentSessionListState();
  const {
    loadSessions,
    loadMoreSessions,
    ensureChildrenLoaded,
    deleteSession,
    deleteSessionOnly,
    toggleBookmark,
  } = useAgentSessionListActions();
  const [activeStatusFilter, setActiveStatusFilter] = useState<
    'all' | SessionStatus
  >('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchRefreshToken, setSearchRefreshToken] = useState(0);

  const showBookmarkedOnly = location.hash === '#bookmarked-sessions';

  const handleShowBookmarkedOnlyChange = useCallback(
    (value: boolean) => {
      if (value) {
        navigate('/history#bookmarked-sessions', { replace: true });
      } else {
        navigate('/history', { replace: true });
      }
    },
    [navigate],
  );

  const handleBrowseLoadMore = useCallback(() => {
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

  const {
    displaySessions,
    hasMoreSessions: displayHasMore,
    isLoading: displayLoading,
    isLoadingMore: displayLoadingMore,
    clientSearchQuery,
    isServerSearchActive,
    loadMore: handleDisplayLoadMore,
    removeSearchSessionTree,
    removeSearchSessionOnly,
    setSearchSessionBookmarked,
  } = useSessionHistorySearch({
    searchQuery,
    browseSessions: sessions,
    browseHasMore: hasMoreSessions,
    browseLoading: isSessionsListLoading,
    browseLoadingMore: isLoadingMoreSessions,
    onBrowseLoadMore: handleBrowseLoadMore,
    refreshToken: searchRefreshToken,
  });

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
        if (isServerSearchActive) {
          removeSearchSessionTree(sessionId);
        }
        toast.success(t('sessionHistory.toasts.deleted', 'Session deleted'));
      } catch (error) {
        logger.error('Failed to delete session', error);
        toast.error(
          t('sessionHistory.toasts.deleteFailed', 'Failed to delete session'),
        );
      }
    },
    [deleteSession, isServerSearchActive, removeSearchSessionTree, t],
  );

  const handleDeleteSessionOnly = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSessionOnly(sessionId);
        if (isServerSearchActive) {
          removeSearchSessionOnly(sessionId);
        }
        toast.success(t('sessionHistory.toasts.deleted', 'Session deleted'));
      } catch (error) {
        logger.error('Failed to delete session only', error);
        toast.error(
          t('sessionHistory.toasts.deleteFailed', 'Failed to delete session'),
        );
      }
    },
    [deleteSessionOnly, isServerSearchActive, removeSearchSessionOnly, t],
  );

  const handleToggleBookmark = useCallback(
    async (sessionId: string) => {
      const currentBookmarked =
        displaySessions.find((session) => session.id === sessionId)
          ?.isBookmarked ?? false;
      const nextBookmarked = !currentBookmarked;

      if (isServerSearchActive) {
        setSearchSessionBookmarked(sessionId, nextBookmarked);
      }

      try {
        await toggleBookmark(sessionId, currentBookmarked);
      } catch (error) {
        if (isServerSearchActive) {
          setSearchSessionBookmarked(sessionId, currentBookmarked);
        }
        logger.error('Failed to toggle bookmark', error);
        toast.error(
          t(
            'sessionHistory.toasts.bookmarkFailed',
            'Failed to update bookmark',
          ),
        );
      }
    },
    [
      displaySessions,
      isServerSearchActive,
      setSearchSessionBookmarked,
      t,
      toggleBookmark,
    ],
  );

  const handleRefreshSessions = useCallback(() => {
    loadSessions(true);
    setSearchRefreshToken((token) => token + 1);
  }, [loadSessions]);

  const handleLoadMoreSessions = useCallback(() => {
    void Promise.resolve(handleDisplayLoadMore()).catch((error) => {
      logger.error('Failed to load more sessions', error);
      toast.error(
        t(
          'sessionHistory.toasts.loadMoreFailed',
          'Failed to load more sessions',
        ),
      );
    });
  }, [handleDisplayLoadMore, t]);

  const handleEnsureChildrenLoaded = useCallback(
    (sessionId: string) => {
      void ensureChildrenLoaded(sessionId).catch((error) => {
        logger.error('Failed to load child sessions', { sessionId, error });
        toast.error(
          t(
            'sessionHistory.toasts.loadChildrenFailed',
            'Failed to load child sessions',
          ),
        );
      });
    },
    [ensureChildrenLoaded, t],
  );

  return (
    <div className="flex min-h-full w-full flex-col text-foreground">
      <SessionHistoryPanel
        sessions={displaySessions}
        isLoading={displayLoading}
        hasMoreSessions={displayHasMore}
        isLoadingMoreSessions={displayLoadingMore}
        activeStatusFilter={activeStatusFilter}
        searchQuery={searchQuery}
        clientSearchQuery={clientSearchQuery}
        onActiveStatusFilterChange={setActiveStatusFilter}
        onSearchQueryChange={setSearchQuery}
        onRefresh={handleRefreshSessions}
        onLoadMore={handleLoadMoreSessions}
        onEnsureChildrenLoaded={handleEnsureChildrenLoaded}
        loadingChildrenParentIds={loadingChildrenParentIds}
        onResume={handleResumeSession}
        onDelete={handleDeleteSession}
        onDeleteOnly={handleDeleteSessionOnly}
        onToggleBookmark={handleToggleBookmark}
        showBookmarkedOnly={showBookmarkedOnly}
        onShowBookmarkedOnlyChange={handleShowBookmarkedOnlyChange}
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
