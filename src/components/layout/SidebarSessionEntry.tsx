import { BookmarkCheck, Circle, Eye, GitBranch } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import { Badge } from '../ui/badge';
import { SidebarMenuButton } from '../ui/sidebar';
import type { AgentSession } from '@/models/agent';

/** Maps session status to a semantically meaningful dot */
export function StatusDot({ status }: { status: string }) {
  if (status === 'busy') {
    // Pulsing to signal active work
    return (
      <Circle
        size={8}
        className="fill-primary text-primary flex-shrink-0 animate-pulse"
      />
    );
  }
  if (status === 'queued') {
    return (
      <Circle
        size={8}
        className="fill-warning text-warning flex-shrink-0 animate-pulse"
      />
    );
  }
  if (status === 'error') {
    return (
      <Circle
        size={8}
        className="fill-destructive text-destructive flex-shrink-0"
      />
    );
  }
  // idle and paused: dimmed — just resting
  return (
    <Circle
      size={8}
      className="fill-muted-foreground text-muted-foreground flex-shrink-0 opacity-40"
    />
  );
}

interface SidebarSessionEntryProps {
  session: AgentSession;
  isActive: boolean;
}

/**
 * One Recent Sessions entry. A session owned by another frontend process
 * (#2759) is shown with an "Open elsewhere" badge and is not a link: the
 * backend refuses entry, so the sidebar must not offer it.
 */
export function SidebarSessionEntry({
  session,
  isActive,
}: SidebarSessionEntryProps) {
  const { t } = useTranslation();
  const label =
    session.name || `${t('sidebar.session')} ${session.id.slice(0, 8)}`;

  if (session.openElsewhere) {
    return (
      <SidebarMenuButton
        disabled
        aria-label={t(
          'sessionHistory.actions.openElsewhereAria',
          'Session {{name}} is open in another process',
          { name: label },
        )}
        className="min-w-0 flex-1"
      >
        <StatusDot status={session.status} />
        <span className="truncate text-xs">{label}</span>
        <Badge
          variant="outline"
          className="h-5 shrink-0 gap-1 px-1.5 text-[10px] text-muted-foreground"
        >
          <Eye className="h-3 w-3" aria-hidden="true" />
          {t('sessionHistory.card.openElsewhere', 'Open elsewhere')}
        </Badge>
      </SidebarMenuButton>
    );
  }

  return (
    <SidebarMenuButton
      asChild
      isActive={isActive}
      tooltip={label}
      className="min-w-0 flex-1"
    >
      <Link
        to={`/agent/${session.id}`}
        className="flex min-w-0 items-center gap-2"
      >
        <StatusDot status={session.status} />
        <span className="truncate text-xs">{label}</span>
        {session.forkedFrom && (
          <Badge
            variant="outline"
            className="h-5 shrink-0 gap-1 px-1.5 text-[10px] text-muted-foreground"
            title={t(
              'sessionHistory.card.forkedFrom',
              'Forked from session {{id}}',
              { id: session.forkedFrom },
            )}
          >
            <GitBranch className="h-3 w-3" aria-hidden="true" />
            {t('sessionHistory.card.forked', 'Forked')}
          </Badge>
        )}
        {session.isBookmarked && (
          <BookmarkCheck className="h-3.5 w-3.5 shrink-0 text-warning" />
        )}
      </Link>
    </SidebarMenuButton>
  );
}
