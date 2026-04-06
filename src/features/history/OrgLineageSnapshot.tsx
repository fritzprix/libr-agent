import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import type { AgentSession } from '@/models/agent';
import { getStatusBadgeConfig } from './org-status';

const DEPTH_CLASS = ['ml-0', 'ml-3', 'ml-6', 'ml-9', 'ml-12'] as const;
const MAX_VISIBLE = 5;

interface OrgLineageSnapshotProps {
  members: AgentSession[];
  orgRootSessionId: string;
}

export function OrgLineageSnapshot({
  members,
  orgRootSessionId,
}: OrgLineageSnapshotProps) {
  const { t } = useTranslation('common');
  const visible = members.slice(0, MAX_VISIBLE);
  const overflow = members.length - MAX_VISIBLE;

  return (
    <div className="space-y-3">
      <div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
        {t('orgHistory.orgChart', 'Lineage Snapshot')}
      </div>
      <div className="space-y-2">
        {visible.map((member) => {
          const depth = Math.min(member.depth ?? 0, 4);
          const isRoot = member.id === orgRootSessionId;
          const badge = getStatusBadgeConfig(member.status);

          return (
            <div
              key={member.id}
              className={cn(
                DEPTH_CLASS[depth],
                'flex items-center justify-between gap-3 rounded-xl border p-3 transition-colors',
                isRoot
                  ? 'border-primary/25 bg-primary/5'
                  : 'border-border/70 bg-background/80',
              )}
            >
              <div className="flex min-w-0 items-start gap-3">
                <span
                  className={cn(
                    'mt-1 h-2.5 w-2.5 shrink-0 rounded-full',
                    isRoot
                      ? 'bg-primary'
                      : member.status === 'busy'
                        ? 'bg-warning'
                        : member.status === 'error'
                          ? 'bg-destructive'
                          : 'bg-muted-foreground/40',
                  )}
                  aria-hidden="true"
                />
                <div className="min-w-0">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate text-sm font-medium">
                      {member.name ?? member.id}
                    </span>
                    {isRoot && (
                      <Badge variant="secondary" className="shrink-0">
                        {t('orgHistory.rootBadge', 'Root')}
                      </Badge>
                    )}
                  </div>
                  <div className="mt-1 truncate text-xs text-muted-foreground">
                    {t('orgHistory.depthLabel', 'Depth')} {member.depth ?? 0} •{' '}
                    {member.id}
                  </div>
                </div>
              </div>

              <Badge
                variant="outline"
                className={cn('shrink-0', badge.className)}
              >
                {t(`sessionHistory.status.${member.status}`, badge.label)}
              </Badge>
            </div>
          );
        })}

        {overflow > 0 && (
          <div className="rounded-xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
            {t('orgHistory.moreMembers', '+{{count}} more members', {
              count: overflow,
            })}
          </div>
        )}
      </div>
    </div>
  );
}
