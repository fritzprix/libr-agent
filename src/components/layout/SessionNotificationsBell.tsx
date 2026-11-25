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

/**
 * Dismissing a row removes the focused menu item. Move focus to the item now at
 * the same position (or the menu itself) so keyboard users stay in the menu.
 *
 * Two traps fixed in #2972 (#2731 2565 N-2): Radix synchronously re-dumps
 * focus onto the first item ("Mark all read") when the roving item is removed,
 * so an "if focus is still in the menu, leave it alone" guard locks focus onto
 * that wrong item. The row only leaves the DOM once `markSessionViewed`
 * resolves and the list re-renders, so focus is restored when the dismissal
 * actually happens — after the promise settles and the item disconnects —
 * instead of a frame budget that gives up while the backend is still slow
 * (#2972 review G1.2).
 */
function keepMenuFocusAfterRemoval(
  item: EventTarget | null,
  dismissal: Promise<unknown>,
) {
  if (!(item instanceof HTMLElement)) return;
  const menu = item.closest<HTMLElement>('[role="menu"]');
  if (!menu) return;
  const menuItems = () =>
    Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]'));
  const index = menuItems().indexOf(item);
  const restore = () => {
    const remaining = menuItems();
    (remaining[Math.min(index, remaining.length - 1)] ?? menu).focus();
  };
  void dismissal.then(
    () => {
      if (!item.isConnected) {
        restore();
        return;
      }
      // The list re-render lands after the promise settles; restore focus the
      // moment the dismissed row actually leaves the DOM.
      const observer = new MutationObserver(() => {
        if (!item.isConnected) {
          observer.disconnect();
          restore();
        } else if (!menu.isConnected) {
          observer.disconnect();
        }
      });
      observer.observe(menu, { childList: true, subtree: true });
    },
    // Dismissal failed: the row stays, so leave focus where the user is.
    () => undefined,
  );
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
        {/* Every control is a menu item (no button nested in a menuitem), so
            arrow keys reach "Mark all read" and each row's dismiss (#2537). */}
        <div className="flex items-center justify-between">
          <DropdownMenuLabel>{t('notifications.title')}</DropdownMenuLabel>
          {notificationSessions.length > 0 ? (
            <DropdownMenuItem
              className="text-muted-foreground focus:text-foreground px-2 py-1 text-xs"
              onSelect={(event) => {
                // Keep the menu open so the user sees the list update.
                event.preventDefault();
                void markAllNotificationsViewed();
              }}
            >
              {t('notifications.markAllRead')}
            </DropdownMenuItem>
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
              <div key={session.id} className="flex items-start gap-1">
                <DropdownMenuItem
                  className="min-w-0 flex-1 items-start py-2"
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
                </DropdownMenuItem>
                {/* A sibling item, not a button inside the row: the row's
                  pointer handling never sees it, so it cannot open the session. */}
                <DropdownMenuItem
                  className="text-muted-foreground focus:text-foreground mt-1 h-7 w-7 shrink-0 justify-center p-0"
                  aria-label={t('notifications.dismiss')}
                  onSelect={(event) => {
                    event.preventDefault();
                    keepMenuFocusAfterRemoval(
                      event.target,
                      markSessionViewed(session.id),
                    );
                  }}
                >
                  <X size={14} />
                </DropdownMenuItem>
              </div>
            );
          })
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
