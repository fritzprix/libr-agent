import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';

interface SessionHistoryEmptyStateProps {
  selectedLineageId: string | null;
  searchQuery: string;
  showBookmarkedOnly: boolean;
  emptyTitle: string;
  emptySubtitle: string;
  onClearLineage: () => void;
  onClearSearch: () => void;
  onClearBookmarkFilter: () => void;
}

/** Empty-list message that explains which focus or filter hides sessions. */
export function SessionHistoryEmptyState({
  selectedLineageId,
  searchQuery,
  showBookmarkedOnly,
  emptyTitle,
  emptySubtitle,
  onClearLineage,
  onClearSearch,
  onClearBookmarkFilter,
}: SessionHistoryEmptyStateProps) {
  const { t } = useTranslation('common');

  return (
    <div className="flex h-full items-center justify-center">
      <div className="text-center text-muted-foreground">
        {selectedLineageId ? (
          <>
            <p className="text-sm">
              {t(
                'sessionHistory.noSessionsInLineage',
                'No sessions visible in lineage {{id}}',
                { id: selectedLineageId.slice(0, 8) },
              )}
            </p>
            <Button
              variant="link"
              size="sm"
              onClick={onClearLineage}
              className="mt-2"
            >
              {t('sessionHistory.clearLineageFocus', 'Clear lineage focus')}
            </Button>
          </>
        ) : searchQuery.trim() ? (
          <>
            <p className="text-sm">
              {t(
                'sessionHistory.noSessionsMatching',
                'No sessions found matching "{{query}}"',
                { query: searchQuery },
              )}
            </p>
            <Button
              variant="link"
              size="sm"
              onClick={onClearSearch}
              className="mt-2"
            >
              {t('sessionHistory.clearSearch', 'Clear search')}
            </Button>
          </>
        ) : showBookmarkedOnly ? (
          <>
            <p className="text-sm">
              {t(
                'sessionHistory.bookmarkFilter.empty',
                'No bookmarked sessions yet',
              )}
            </p>
            <Button
              variant="link"
              size="sm"
              onClick={onClearBookmarkFilter}
              className="mt-2"
            >
              {t('sessionHistory.bookmarkFilter.clear', 'Show all sessions')}
            </Button>
          </>
        ) : (
          <>
            <p className="text-sm">{emptyTitle}</p>
            <p className="mt-2 text-xs">{emptySubtitle}</p>
          </>
        )}
      </div>
    </div>
  );
}
