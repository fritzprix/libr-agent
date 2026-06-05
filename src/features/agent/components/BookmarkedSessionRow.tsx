import { StarOff, Clock3 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { formatRelativeTime } from '@/lib/date-utils';
import type { AgentSession } from '@/models/agent';
import { getLatestSessionActivityTimestamp } from '@/lib/session-metadata';
import {
  getSessionDisplayName,
  type SessionHistoryTranslate,
} from './session-history-utils';

export interface BookmarkedSessionTileProps {
  session: AgentSession;
  onResume: (sessionId: string) => void;
  onToggleBookmark?: (sessionId: string) => void;
  t: SessionHistoryTranslate;
}

export function BookmarkedSessionRow({
  session,
  onResume,
  onToggleBookmark,
  t,
}: BookmarkedSessionTileProps) {
  const shortcutLabel = getSessionDisplayName(session, t);
  const latestActivity =
    formatRelativeTime(
      new Date(getLatestSessionActivityTimestamp(session)),
      new Date(),
    ) || t('sessionHistory.card.justNow', 'just now');
  const secondaryLabel =
    session.assistant?.name ||
    (session.provider && session.model
      ? `${session.provider}/${session.model}`
      : t(
          'sessionHistory.bookmarkedSection.defaultMeta',
          'Saved for quick access',
        ));

  return (
    <div className="flex items-center gap-2 rounded-lg border bg-background/80 px-3 py-2 shadow-sm shadow-black/5">
      <Button
        type="button"
        variant="ghost"
        className="-my-1 h-auto min-w-0 flex-1 justify-start px-2 py-1"
        onClick={() => onResume(session.id)}
        aria-label={t(
          'sessionHistory.bookmarkedSection.resumeAria',
          'Open bookmarked session {{name}}',
          { name: shortcutLabel },
        )}
      >
        <div className="min-w-0 flex-1 text-left">
          <div className="truncate text-sm font-semibold leading-5 text-foreground">
            {shortcutLabel}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <span className="truncate">{secondaryLabel}</span>
            <span aria-hidden="true">•</span>
            <Clock3 className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">
              {t('sessionHistory.bookmarkedSection.lastUsed', 'Used {{time}}', {
                time: latestActivity,
              })}
            </span>
          </div>
        </div>
      </Button>
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="h-8 w-8 shrink-0"
        onClick={() => onToggleBookmark?.(session.id)}
        aria-label={t(
          'sessionHistory.actions.unbookmarkAria',
          'Remove bookmark',
        )}
      >
        <StarOff className="h-4 w-4" aria-hidden="true" />
      </Button>
    </div>
  );
}
