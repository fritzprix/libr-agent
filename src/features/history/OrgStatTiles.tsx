import { Activity, Clock3, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { formatSessionTimestamp } from '@/lib/date-utils';

interface OrgStatTilesProps {
  memberCount: number;
  busyCount: number;
  updatedAt: Date;
}

export function OrgStatTiles({
  memberCount,
  busyCount,
  updatedAt,
}: OrgStatTilesProps) {
  const { t } = useTranslation('common');
  const ts = formatSessionTimestamp(updatedAt);

  return (
    <div className="grid gap-3 sm:grid-cols-3">
      <div className="rounded-xl border bg-background/80 p-3">
        <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          <Users className="h-3.5 w-3.5" />
          {t('orgHistory.membersLabel', 'Members')}
        </div>
        <div className="text-2xl font-semibold">{memberCount}</div>
        <div className="text-xs text-muted-foreground">
          {t('orgHistory.members', '{{count}} members', { count: memberCount })}
        </div>
      </div>

      <div className="rounded-xl border bg-background/80 p-3">
        <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          <Activity className="h-3.5 w-3.5" />
          {t('orgHistory.activeLabel', 'Active')}
        </div>
        <div className="text-2xl font-semibold">{busyCount}</div>
        <div className="text-xs text-muted-foreground">
          {t('orgHistory.busy', '{{count}} busy', { count: busyCount })}
        </div>
      </div>

      <div className="rounded-xl border bg-background/80 p-3">
        <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          <Clock3 className="h-3.5 w-3.5" />
          {t('orgHistory.updatedLabel', 'Updated')}
        </div>
        <div className="truncate text-sm font-semibold">
          {ts.relative ?? ts.display}
        </div>
        <div
          className="truncate text-xs text-muted-foreground"
          title={ts.tooltip}
        >
          {ts.display}
        </div>
      </div>
    </div>
  );
}
