import { useTranslation } from 'react-i18next';
import { History, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { HISTORY_CONTENT_RAIL_CLASS } from '../session-history-utils';

interface SessionHistoryHeaderProps {
  heading: string;
  description: string;
  showBookmarkedOnly: boolean;
  baseSessionCount: number;
  visibleRowCount: number;
  hasMoreSessions: boolean;
  filtersActive: boolean;
  matchedSessionCount: number;
  isLoading: boolean;
  onRefresh: () => void;
}

export function SessionHistoryHeader({
  heading,
  description,
  showBookmarkedOnly,
  baseSessionCount,
  visibleRowCount,
  hasMoreSessions,
  filtersActive,
  matchedSessionCount,
  isLoading,
  onRefresh,
}: SessionHistoryHeaderProps) {
  const { t } = useTranslation('common');

  return (
    <div
      className={cn(
        HISTORY_CONTENT_RAIL_CLASS,
        'mb-8 flex items-center justify-between gap-3',
      )}
    >
      <div className="flex items-center gap-4">
        <div className="rounded-xl bg-primary/10 p-2.5 text-primary">
          <History size={28} />
        </div>
        <div>
          <h1
            className="text-2xl font-semibold tracking-tight text-foreground"
            id="session-heading"
          >
            {heading}
          </h1>
          <p className="mt-0.5 text-sm text-muted-foreground">
            {description} (
            {showBookmarkedOnly
              ? t(
                  'sessionHistory.bookmarkedCountSummary',
                  '{{count}} bookmarked sessions',
                  {
                    count: baseSessionCount,
                  },
                )
              : t(
                  'sessionHistory.loadedCountSummary',
                  '{{count}} loaded sessions',
                  {
                    count: baseSessionCount,
                  },
                )}
            {visibleRowCount !== baseSessionCount
              ? `, ${t(
                  'sessionHistory.visibleCountSummary',
                  '{{count}} visible in tree',
                  {
                    count: visibleRowCount,
                  },
                )}`
              : ''}
            {hasMoreSessions
              ? `, ${t('sessionHistory.moreAvailable', 'more available')}`
              : ''}
            {filtersActive
              ? `, ${t(
                  'sessionHistory.matchCountSummary',
                  '{{count}} matching filters',
                  { count: matchedSessionCount },
                )}`
              : ''}
            )
          </p>
        </div>
      </div>
      <Button
        variant="ghost"
        size="icon"
        onClick={onRefresh}
        disabled={isLoading}
        aria-label={t('sessionHistory.refreshAria', 'Refresh sessions')}
        className="h-9 w-9"
      >
        <RefreshCw className={cn('h-4 w-4', isLoading && 'animate-spin')} />
      </Button>
    </div>
  );
}
