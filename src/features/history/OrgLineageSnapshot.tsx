import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import type { AgentSession } from '@/models/agent';
import {
  getStatusBadgeConfig,
  getStatusDotClass,
  getStatusNodeClass,
} from './org-status';

const MAX_VISIBLE = 8;

interface OrgLineageSnapshotProps {
  rootSession: AgentSession;
  members: AgentSession[];
  orgRootSessionId: string;
}

interface SessionNodeProps {
  member: AgentSession;
  isRoot?: boolean;
}

function SessionNode({ member, isRoot = false }: SessionNodeProps) {
  const { t } = useTranslation('common');
  const badge = getStatusBadgeConfig(member.status);
  const displayName = member.name ?? member.id;

  return (
    <div
      className={cn(
        'min-w-[10rem] max-w-[13rem] rounded-2xl border px-4 py-3 shadow-sm shadow-black/5 transition-colors',
        isRoot
          ? 'border-primary/25 bg-primary/5'
          : getStatusNodeClass(member.status),
      )}
      title={`${displayName} • ${member.id}`}
    >
      <div className="flex items-center gap-2">
        <span
          className={cn(
            'h-2.5 w-2.5 shrink-0 rounded-full',
            isRoot ? 'bg-primary' : getStatusDotClass(member.status),
          )}
          aria-hidden="true"
        />
        <span className="truncate text-sm font-medium text-foreground">
          {displayName}
        </span>
      </div>
      <div className="mt-2 truncate text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
        {isRoot
          ? t('orgHistory.rootBadge', 'Root')
          : t(`sessionHistory.status.${member.status}`, badge.label)}
      </div>
    </div>
  );
}

export function OrgLineageSnapshot({
  rootSession,
  members,
  orgRootSessionId,
}: OrgLineageSnapshotProps) {
  const { t } = useTranslation('common');
  const descendantMembers = members.filter(
    (member) => member.id !== orgRootSessionId,
  );
  const visible = descendantMembers.slice(0, MAX_VISIBLE);
  const overflow = Math.max(0, descendantMembers.length - MAX_VISIBLE);
  const membersByDepth = new Map<number, AgentSession[]>();

  for (const member of visible) {
    const depth = Math.max(1, member.depth ?? 1);
    const layer = membersByDepth.get(depth) ?? [];
    layer.push(member);
    membersByDepth.set(depth, layer);
  }

  const rows = [...membersByDepth.entries()].sort(
    ([left], [right]) => left - right,
  );

  return (
    <div className="rounded-2xl border border-border/70 bg-muted/10 p-4">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
          {t('orgHistory.orgChart', 'Org Chart')}
        </div>
        {overflow > 0 && (
          <div className="text-xs text-muted-foreground">
            {t('orgHistory.moreMembers', '+{{count}} more members', {
              count: overflow,
            })}
          </div>
        )}
      </div>

      <div className="space-y-4">
        <div className="flex justify-center">
          <SessionNode member={rootSession} isRoot />
        </div>

        {rows.length === 0 ? (
          <div className="rounded-xl border border-dashed border-border/70 px-4 py-3 text-center text-sm text-muted-foreground">
            {t('orgHistory.noAdditionalMembers', 'No additional members yet')}
          </div>
        ) : (
          <div className="space-y-4">
            {rows.map(([depth, row]) => (
              <div key={depth} className="space-y-3">
                <div
                  className="mx-auto h-4 w-px bg-border/70"
                  aria-hidden="true"
                />
                <div
                  className="mx-auto h-px w-full max-w-xl bg-border/50"
                  aria-hidden="true"
                />
                <div className="flex flex-wrap justify-center gap-3">
                  {row.map((member) => (
                    <SessionNode key={member.id} member={member} />
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
