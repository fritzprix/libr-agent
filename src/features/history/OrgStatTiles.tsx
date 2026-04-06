import { Activity, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface OrgStatTilesProps {
  memberCount: number;
  busyCount: number;
}

export function OrgStatTiles({ memberCount, busyCount }: OrgStatTilesProps) {
  const { t } = useTranslation('common');

  return (
    <div className="flex flex-wrap gap-2">
      <div className="inline-flex items-center gap-3 rounded-full border border-border/70 bg-background/80 px-4 py-2">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          <Users className="h-3.5 w-3.5" />
          {t('orgHistory.membersLabel', 'Members')}
        </div>
        <div className="text-sm font-semibold tabular-nums text-foreground">
          {memberCount}
        </div>
      </div>

      <div className="inline-flex items-center gap-3 rounded-full border border-border/70 bg-background/80 px-4 py-2">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          <Activity className="h-3.5 w-3.5" />
          {t('orgHistory.activeLabel', 'Active')}
        </div>
        <div className="text-sm font-semibold tabular-nums text-foreground">
          {busyCount}
        </div>
      </div>
    </div>
  );
}
