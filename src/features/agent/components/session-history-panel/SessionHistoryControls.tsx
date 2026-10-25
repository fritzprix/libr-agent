import { useTranslation } from 'react-i18next';
import { BookmarkCheck, ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { SessionStatus, SessionStatusCounts } from '@/lib/session-utils';
import {
  isSessionSortKey,
  isSessionStatusFilterValue,
  sessionSortValues,
  statusFilterValues,
  type SessionSortDirection,
  type SessionSortKey,
} from '../session-history-utils';

interface SessionHistoryControlsProps {
  searchPlaceholder: string;
  searchQuery: string;
  onSearchQueryChange: (value: string) => void;
  showBookmarkedOnly: boolean;
  onToggleShowBookmarkedOnly: () => void;
  bookmarkedCount: number;
  activeSortKey: SessionSortKey;
  onActiveSortKeyChange: (value: SessionSortKey) => void;
  activeSortDirection: SessionSortDirection;
  onToggleSortDirection: () => void;
  activeStatusFilter: 'all' | SessionStatus;
  onActiveStatusFilterChange: (value: 'all' | SessionStatus) => void;
  statusCounts: SessionStatusCounts;
}

/** Search box, bookmark toggle, sort, and status filter row. */
export function SessionHistoryControls({
  searchPlaceholder,
  searchQuery,
  onSearchQueryChange,
  showBookmarkedOnly,
  onToggleShowBookmarkedOnly,
  bookmarkedCount,
  activeSortKey,
  onActiveSortKeyChange,
  activeSortDirection,
  onToggleSortDirection,
  activeStatusFilter,
  onActiveStatusFilterChange,
  statusCounts,
}: SessionHistoryControlsProps) {
  const { t } = useTranslation('common');

  const statusFilterLabelByValue: Record<'all' | SessionStatus, string> = {
    all: t('sessionHistory.statusFilter.all', 'All statuses'),
    busy: `${t('sessionHistory.tabs.busy', 'Busy')} (${statusCounts.busy})`,
    idle: `${t('sessionHistory.tabs.idle', 'Idle')} (${statusCounts.idle})`,
    paused: `${t('sessionHistory.tabs.paused', 'Paused')} (${statusCounts.paused})`,
    error: `${t('sessionHistory.tabs.error', 'Error')} (${statusCounts.error})`,
    queued: `${t('sessionHistory.tabs.queued', 'Queued')} (${statusCounts.queued})`,
    provisioning: `${t('sessionHistory.tabs.provisioning', 'Setting up')} (${statusCounts.provisioning})`,
  };
  const sortLabelByValue: Record<SessionSortKey, string> = {
    updatedAt: t('sessionHistory.sort.updatedAt', 'Recent activity'),
    createdAt: t('sessionHistory.sort.createdAt', 'Created'),
    name: t('sessionHistory.sort.name', 'Name'),
  };
  const sortDirectionToggleLabel =
    activeSortDirection === 'asc'
      ? t('sessionHistory.sort.descending', 'Sort descending')
      : t('sessionHistory.sort.ascending', 'Sort ascending');

  return (
    <div className="flex flex-col gap-3 md:flex-row md:items-center">
      <div className="relative min-w-0 flex-1">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 transform text-muted-foreground" />
        <Input
          type="text"
          placeholder={searchPlaceholder}
          value={searchQuery}
          onChange={(event) => onSearchQueryChange(event.target.value)}
          className="pl-10 pr-10"
          aria-label={t('sessionHistory.searchAria', 'Search sessions')}
        />
        {searchQuery && (
          <button
            type="button"
            onClick={() => onSearchQueryChange('')}
            className="absolute right-3 top-1/2 -translate-y-1/2 transform rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label={t('sessionHistory.clearSearchAria', 'Clear search')}
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2 md:justify-end">
        <Button
          type="button"
          size="sm"
          variant={showBookmarkedOnly ? 'secondary' : 'outline'}
          className="shrink-0"
          aria-pressed={showBookmarkedOnly}
          onClick={onToggleShowBookmarkedOnly}
        >
          <BookmarkCheck className="mr-2 h-4 w-4" />
          {t('sessionHistory.tabs.bookmarked', 'Bookmarked')} ({bookmarkedCount}
          )
        </Button>
      </div>

      <div className="flex items-center gap-2 md:w-64 md:shrink-0">
        <Select
          value={activeSortKey}
          onValueChange={(value) => {
            if (isSessionSortKey(value)) {
              onActiveSortKeyChange(value);
            }
          }}
        >
          <SelectTrigger
            size="sm"
            className="w-full"
            aria-label={t('sessionHistory.sort.label', 'Sort sessions')}
          >
            <SelectValue>{sortLabelByValue[activeSortKey]}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {sessionSortValues.map((value) => (
              <SelectItem key={value} value={value}>
                {sortLabelByValue[value]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-8 w-8 shrink-0"
          onClick={onToggleSortDirection}
          aria-label={sortDirectionToggleLabel}
        >
          {activeSortDirection === 'asc' ? (
            <ChevronUp className="h-4 w-4" />
          ) : (
            <ChevronDown className="h-4 w-4" />
          )}
        </Button>
      </div>

      <div className="md:w-56 md:shrink-0">
        <Select
          value={activeStatusFilter}
          onValueChange={(value) => {
            if (isSessionStatusFilterValue(value)) {
              onActiveStatusFilterChange(value);
            }
          }}
        >
          <SelectTrigger
            size="sm"
            className="w-full"
            aria-label={t('sessionHistory.statusFilter.label', 'Status')}
          >
            <SelectValue>
              {statusFilterLabelByValue[activeStatusFilter]}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {statusFilterValues.map((value) => (
              <SelectItem key={value} value={value}>
                {statusFilterLabelByValue[value]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}
