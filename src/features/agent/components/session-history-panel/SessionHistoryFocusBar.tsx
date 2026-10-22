import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { HISTORY_CONTENT_RAIL_CLASS } from '../session-history-utils';

interface SessionHistoryFocusBarProps {
  selectedLineageId: string | null;
  showBookmarkedOnly: boolean;
  onClearLineage: () => void;
  onClearBookmarkFilter: () => void;
}

/** Shows the active lineage / bookmark focus with links to clear each. */
export function SessionHistoryFocusBar({
  selectedLineageId,
  showBookmarkedOnly,
  onClearLineage,
  onClearBookmarkFilter,
}: SessionHistoryFocusBarProps) {
  const { t } = useTranslation('common');

  return (
    <div
      className={cn(
        HISTORY_CONTENT_RAIL_CLASS,
        'mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground',
      )}
    >
      {selectedLineageId && (
        <>
          <span>
            {t('sessionHistory.focusedLineage', 'Focused lineage: {{id}}', {
              id: selectedLineageId.slice(0, 8),
            })}
          </span>
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0"
            onClick={onClearLineage}
          >
            {t('sessionHistory.showAllLineages', 'Show all lineages')}
          </Button>
        </>
      )}
      {showBookmarkedOnly && (
        <>
          <span>
            {t(
              'sessionHistory.bookmarkFilter.focused',
              'Showing bookmarked sessions',
            )}
          </span>
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0"
            onClick={onClearBookmarkFilter}
          >
            {t('sessionHistory.bookmarkFilter.clear', 'Show all sessions')}
          </Button>
        </>
      )}
    </div>
  );
}
