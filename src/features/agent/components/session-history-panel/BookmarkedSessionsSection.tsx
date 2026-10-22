import { useTranslation } from 'react-i18next';
import { BookmarkCheck } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { AgentSession } from '@/models/agent';
import { BookmarkedSessionRow } from '../BookmarkedSessionRow';
import {
  HISTORY_CONTENT_RAIL_CLASS,
  HISTORY_SECTION_CLASS,
} from '../session-history-utils';

interface BookmarkedSessionsSectionProps {
  totalCount: number;
  featuredSessions: AgentSession[];
  remainingCount: number;
  onBrowseAll: () => void;
  onResume: (sessionId: string) => void;
  onToggleBookmark?: (sessionId: string) => void;
}

export function BookmarkedSessionsSection({
  totalCount,
  featuredSessions,
  remainingCount,
  onBrowseAll,
  onResume,
  onToggleBookmark,
}: BookmarkedSessionsSectionProps) {
  const { t } = useTranslation('common');

  return (
    <section
      id="bookmarked-sessions"
      className={cn(HISTORY_CONTENT_RAIL_CLASS, HISTORY_SECTION_CLASS, 'mb-4')}
    >
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <BookmarkCheck className="h-4 w-4 text-warning" />
            <h2 className="text-sm font-semibold text-foreground">
              {t(
                'sessionHistory.bookmarkedSection.heading',
                'Bookmarked Sessions',
              )}
            </h2>
            <Badge variant="secondary">{totalCount}</Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            {t(
              'sessionHistory.bookmarkedSection.description',
              'Pinned sessions stay here for quick access while the full history remains below.',
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {remainingCount > 0 && (
            <Badge variant="outline" className="h-6 px-2">
              {t(
                'sessionHistory.bookmarkedSection.more',
                '+{{count}} more bookmarked sessions',
                { count: remainingCount },
              )}
            </Badge>
          )}
          {remainingCount > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onBrowseAll}
            >
              {t(
                'sessionHistory.bookmarkedSection.browseAll',
                'Browse all bookmarked sessions',
              )}
            </Button>
          )}
        </div>
      </div>

      <div className="mt-3 flex flex-col gap-2" role="list">
        {featuredSessions.map((session) => (
          <div key={session.id} role="listitem">
            <BookmarkedSessionRow
              session={session}
              onResume={onResume}
              onToggleBookmark={onToggleBookmark}
              t={t}
            />
          </div>
        ))}
      </div>
    </section>
  );
}
