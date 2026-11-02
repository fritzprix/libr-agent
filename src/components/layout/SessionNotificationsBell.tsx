import { useState } from 'react';
import { Bell, X } from 'lucide-react';
import type { DropdownMenuItemProps } from '@radix-ui/react-dropdown-menu';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Badge } from '../ui/badge';
import { Button } from '../ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';

function formatSessionName(
  sessionName: string | undefined,
  sessionId: string,
): string {
  return sessionName || `Session ${sessionId.slice(0, 8)}`;
}

export function SessionNotificationsBell() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const { markSessionViewed, markAllNotificationsViewed } =
    useAgentSessionListActions();
  const { notificationSessions, unreadNotificationCount } =
    useAgentSessionListState();

  const handleSessionSelect = (
    sessionId: string,
  ): NonNullable<DropdownMenuItemProps['onSelect']> => {
    return (event) => {
      event.preventDefault();
      event.stopPropagation();

      setOpen(false);
      void (async () => {
        try {
          await markSessionViewed(sessionId);
        } finally {
          navigate(`/agent/${sessionId}`);
        }
      })();
    };
  };

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="relative"
              aria-label={t('notifications.open')}
            >
              <Bell size={16} />
              {unreadNotificationCount > 0 ? (
                <span className="bg-primary text-primary-foreground absolute -top-1 -right-1 flex min-h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] leading-none">
                  {unreadNotificationCount}
                </span>
              ) : null}
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{t('notifications.open')}</TooltipContent>
      </Tooltip>

      <DropdownMenuContent align="end" className="w-80">
        <div className="flex items-center justify-between pr-2">
          <DropdownMenuLabel>{t('notifications.title')}</DropdownMenuLabel>
          {notificationSessions.length > 0 ? (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground h-auto px-2 py-1 text-xs"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                void markAllNotificationsViewed();
              }}
            >
              {t('notifications.markAllRead')}
            </Button>
          ) : null}
        </div>
        <DropdownMenuSeparator />

        {notificationSessions.length === 0 ? (
          <div className="text-muted-foreground px-2 py-3 text-sm">
            {t('notifications.empty')}
          </div>
        ) : (
          notificationSessions.map((session) => {
            const name = formatSessionName(session.name, session.id);
            const pendingApprovalCount = session.pendingApprovalCount ?? 0;
            const hasRecurringStopAttention = Boolean(
              session.lastAttentionReason === 'recurringStop' &&
                session.lastAttentionAt &&
                (!session.lastViewedAt ||
                  session.lastAttentionAt.getTime() >
                    session.lastViewedAt.getTime()),
            );

            return (
              <DropdownMenuItem
                key={session.id}
                className="items-start py-2"
                onSelect={handleSessionSelect(session.id)}
              >
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <div className="truncate font-medium">{name}</div>
                  <div className="flex flex-wrap gap-1">
                    {hasRecurringStopAttention ? (
                      <Badge variant="secondary">
                        {t('notifications.recurringStop')}
                      </Badge>
                    ) : null}
                    {pendingApprovalCount > 0 ? (
                      <Badge variant="destructive">
                        {t('notifications.pendingApprovals', {
                          count: pendingApprovalCount,
                        })}
                      </Badge>
                    ) : null}
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="text-muted-foreground hover:text-foreground -mr-1 h-6 w-6 shrink-0"
                  aria-label={t('notifications.dismiss')}
                  onPointerDown={(event) => {
                    // Keep the row's DropdownMenuItem from recording the
                    // pointer, so it never treats this as a row click.
                    event.stopPropagation();
                  }}
                  onPointerUp={(event) => {
                    // DropdownMenuItem's onPointerUp runs `currentTarget.click()`
                    // when it did not see the pointerdown. Since the dismiss
                    // button swallows pointerdown, an unstopped pointerup makes
                    // the item synthesize a click on the row -> onSelect ->
                    // navigate to the session. Stop it here too.
                    event.stopPropagation();
                  }}
                  onKeyDown={(event) => {
                    event.stopPropagation();
                  }}
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    void markSessionViewed(session.id);
                  }}
                >
                  <X size={14} />
                </Button>
              </DropdownMenuItem>
            );
          })
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
