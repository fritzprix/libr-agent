import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  Trash2,
  Play,
  Eye,
  Circle,
  Pause,
  XCircle,
  Bookmark,
  BookmarkCheck,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { getLogger } from '@/lib/logger';
import { formatRelativeTime } from '@/lib/date-utils';
import type { AgentSession } from '@/models/agent';
import type { SessionStatusCounts } from '@/lib/session-utils';
import { cn } from '@/lib/utils';

const logger = getLogger('SessionCard');

interface SessionCardProps {
  session: AgentSession;
  onResume: (sessionId: string) => void;
  onDelete: (sessionId: string) => void;
  nestingLevel?: number;
  lineageHint?: string;
  selectedLineageId?: string | null;
  onLineageSelect?: (lineageId: string) => void;
  /** Number of descendant subagent sessions that will also be deleted (SP7). */
  descendantCount?: number;
  /** Descendant status summary for tree-focused UI. */
  descendantStatusCounts?: SessionStatusCounts;
  /** Delete only this session, promoting children to top-level (SP7). */
  onDeleteOnly?: (sessionId: string) => void;
  /** Toggle bookmark on this session (SP12). */
  onToggleBookmark?: (sessionId: string) => void;
  /** Whether this row has nested children that can be expanded. */
  hasExpandableChildren?: boolean;
  /** Whether nested children are currently visible. */
  isExpanded?: boolean;
  /** Toggle child visibility. */
  onToggleExpand?: (sessionId: string) => void;
}

/**
 * SessionCard Component
 *
 * Displays agent session metadata with action buttons.
 * Used in AgentChatStartView's session history panel.
 */
export function SessionCard({
  session,
  onResume,
  onDelete,
  nestingLevel = 0,
  lineageHint,
  selectedLineageId = null,
  onLineageSelect,
  descendantCount = 0,
  descendantStatusCounts,
  onDeleteOnly,
  onToggleBookmark,
  hasExpandableChildren = false,
  isExpanded = false,
  onToggleExpand,
}: SessionCardProps) {
  const [isDeleting, setIsDeleting] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const { t } = useTranslation('common');

  const getStatusConfig = useCallback(
    (status: string) => {
      switch (status) {
        case 'busy':
          return {
            icon: 'active',
            badge: t('sessionHistory.status.busy', 'Active'),
            variant: 'outline' as const,
            className:
              'bg-warning/10 text-warning-foreground border-warning/20',
            cardClassName:
              'border-warning/30 bg-warning/5 shadow-sm shadow-warning/10 hover:bg-warning/10',
            accentClassName: 'bg-warning/70',
          };
        case 'idle':
          return {
            icon: 'idle',
            badge: t('sessionHistory.status.idle', 'Idle'),
            variant: 'secondary' as const,
            className: '',
            cardClassName:
              'border-border/80 bg-card shadow-sm shadow-black/5 hover:bg-muted/40',
            accentClassName: 'bg-muted-foreground/30',
          };
        case 'paused':
          return {
            icon: 'paused',
            badge: t('sessionHistory.status.paused', 'Paused'),
            variant: 'secondary' as const,
            className: 'opacity-75',
            cardClassName:
              'border-muted-foreground/20 bg-muted/35 shadow-sm shadow-black/5 hover:bg-muted/50',
            accentClassName: 'bg-muted-foreground/45',
          };
        case 'error':
          return {
            icon: 'error',
            badge: t('sessionHistory.status.error', 'Error'),
            variant: 'destructive' as const,
            className:
              'bg-destructive/10 text-destructive border-destructive/20',
            cardClassName:
              'border-destructive/30 bg-destructive/5 shadow-sm shadow-destructive/10 hover:bg-destructive/10',
            accentClassName: 'bg-destructive/75',
          };
        default:
          return {
            icon: 'unknown',
            badge: t('sessionHistory.status.unknown', 'Unknown'),
            variant: 'outline' as const,
            className: 'text-muted-foreground',
            cardClassName:
              'border-border/80 bg-card shadow-sm shadow-black/5 hover:bg-muted/40',
            accentClassName: 'bg-muted-foreground/30',
          };
      }
    },
    [t],
  );

  const handleDelete = useCallback(async () => {
    if (!showConfirm) {
      setShowConfirm(true);
      return;
    }

    setIsDeleting(true);
    try {
      await onDelete(session.id);
      logger.info('Session deleted', { sessionId: session.id });
    } catch (err) {
      logger.error('Failed to delete session', err);
    } finally {
      setIsDeleting(false);
      setShowConfirm(false);
    }
  }, [showConfirm, session.id, onDelete]);

  const handleDeleteOnly = useCallback(async () => {
    if (!onDeleteOnly) return;
    setIsDeleting(true);
    try {
      await onDeleteOnly(session.id);
      logger.info('Session deleted (children orphaned)', {
        sessionId: session.id,
      });
    } catch (err) {
      logger.error('Failed to delete session only', err);
    } finally {
      setIsDeleting(false);
      setShowConfirm(false);
    }
  }, [session.id, onDeleteOnly]);

  const handleCancelDelete = useCallback(() => {
    setShowConfirm(false);
  }, []);

  const statusConfig = getStatusConfig(session.status);
  const isActive = session.status === 'busy' || session.status === 'idle';
  const isViewOnly = session.status === 'error';
  const isPaused = session.status === 'paused';
  const shortLineageId = session.lineageId?.slice(0, 8);
  const shortParentId = session.parentSessionId?.slice(0, 8);
  const depthLabel =
    typeof session.depth === 'number' ? `D${session.depth}` : null;
  const relationBadge = session.parentSessionId
    ? t('sessionHistory.card.child', 'Child')
    : t('sessionHistory.card.root', 'Root');
  const isSelectedLineage =
    !!session.lineageId && selectedLineageId === session.lineageId;

  const sessionNameFallback =
    session.name ||
    t('sessionHistory.card.fallbackName', 'Session {{id}}', {
      id: session.id.slice(0, 8),
    });
  const createdAtLabel = t(
    'sessionHistory.card.createdAt',
    'Created {{time}}',
    {
      time:
        formatRelativeTime(session.createdAt, new Date()) ||
        t('sessionHistory.card.justNow', 'just now'),
    },
  );
  const updatedAtLabel = session.updatedAt
    ? t('sessionHistory.card.updatedAt', 'Updated {{time}}', {
        time:
          formatRelativeTime(session.updatedAt, new Date()) ||
          t('sessionHistory.card.justNow', 'just now'),
      })
    : null;
  const summaryParts = [
    session.assistant?.name,
    session.model && session.provider
      ? `${t('sessionHistory.card.model', 'Model:')} ${session.provider}/${session.model}`
      : null,
    lineageHint,
    updatedAtLabel,
    createdAtLabel,
  ].filter((value): value is string => Boolean(value));
  const contentIndentStyle =
    nestingLevel > 0
      ? { paddingLeft: `${Math.min(nestingLevel, 4) * 14}px` }
      : undefined;

  return (
    <article
      className={cn(
        'relative overflow-hidden rounded-xl border px-3 py-2.5 transition',
        statusConfig.cardClassName,
        session.isBookmarked &&
          'ring-1 ring-warning/25 shadow-sm shadow-warning/10',
      )}
      aria-label={t('sessionHistory.card.ariaLabel', 'Session: {{name}}', {
        name: sessionNameFallback,
      })}
    >
      <div
        className={cn(
          'absolute inset-y-0 left-0 w-1 rounded-l-xl',
          statusConfig.accentClassName,
        )}
        aria-hidden="true"
      />
      <div className="space-y-2.5" style={contentIndentStyle}>
        <div className="grid grid-cols-[auto,minmax(0,1fr),auto] items-start gap-2">
          <div className="pt-0.5">
            {hasExpandableChildren ? (
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className="h-6 w-6 shrink-0"
                onClick={() => onToggleExpand?.(session.id)}
                aria-label={
                  isExpanded
                    ? t(
                        'sessionHistory.actions.collapseChildrenAria',
                        'Collapse child sessions for {{name}}',
                        { name: sessionNameFallback },
                      )
                    : t(
                        'sessionHistory.actions.expandChildrenAria',
                        'Expand child sessions for {{name}}',
                        { name: sessionNameFallback },
                      )
                }
              >
                {isExpanded ? (
                  <ChevronDown className="h-4 w-4" aria-hidden="true" />
                ) : (
                  <ChevronRight className="h-4 w-4" aria-hidden="true" />
                )}
              </Button>
            ) : (
              <span className="w-6 shrink-0" aria-hidden="true" />
            )}
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold leading-5">
              {sessionNameFallback}
            </h3>
            {summaryParts.length > 0 && (
              <p className="mt-1 truncate text-xs text-muted-foreground">
                {summaryParts.join(' · ')}
              </p>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-1">
            {session.isBookmarked && (
              <Badge
                variant="secondary"
                className="h-5 border-warning/20 bg-warning/10 px-1.5 text-[10px] text-warning-foreground"
              >
                <BookmarkCheck className="h-3.5 w-3.5" aria-hidden="true" />
                {t('sessionHistory.card.bookmarkedBadge', 'Bookmarked')}
              </Badge>
            )}
            <Badge
              variant={statusConfig.variant}
              className={cn('h-5 px-1.5 text-[10px]', statusConfig.className)}
              role="status"
              aria-label={t(
                'sessionHistory.card.statusAriaLabel',
                'Session status: {{status}}',
                { status: statusConfig.badge },
              )}
            >
              {statusConfig.icon === 'active' && (
                <Circle className="h-3 w-3 fill-current" />
              )}
              {statusConfig.icon === 'idle' && (
                <Circle className="h-3 w-3 fill-current" />
              )}
              {statusConfig.icon === 'paused' && <Pause className="h-3 w-3" />}
              {statusConfig.icon === 'error' && <XCircle className="h-3 w-3" />}
              {statusConfig.icon === 'unknown' && (
                <Circle className="h-3 w-3 fill-current" />
              )}
              {statusConfig.badge}
            </Badge>
          </div>
        </div>

        {(depthLabel ||
          shortParentId ||
          shortLineageId ||
          (descendantStatusCounts && descendantCount > 0)) && (
          <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
            <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
              {relationBadge}
            </Badge>
            {depthLabel && (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {depthLabel}
              </Badge>
            )}
            {descendantCount > 0 && (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t('sessionHistory.card.descendants', '{{count}} descendants', {
                  count: descendantCount,
                })}
              </Badge>
            )}
            {shortParentId && (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t('sessionHistory.card.parentBadge', 'Parent: {{id}}', {
                  id: shortParentId,
                })}
              </Badge>
            )}
            {shortLineageId && session.lineageId && (
              <Button
                type="button"
                size="sm"
                variant={isSelectedLineage ? 'default' : 'outline'}
                className="h-5 px-1.5 text-[10px]"
                onClick={() => onLineageSelect?.(session.lineageId)}
                aria-label={t(
                  'sessionHistory.card.lineageFilterAria',
                  'Filter by lineage {{id}}',
                  { id: shortLineageId },
                )}
              >
                {t('sessionHistory.card.lineageBadge', 'Lineage: {{id}}', {
                  id: shortLineageId,
                })}
              </Button>
            )}
            {descendantStatusCounts?.busy ? (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t('sessionHistory.card.descendantBusy', 'Busy: {{count}}', {
                  count: descendantStatusCounts.busy,
                })}
              </Badge>
            ) : null}
            {descendantStatusCounts?.idle ? (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t('sessionHistory.card.descendantIdle', 'Idle: {{count}}', {
                  count: descendantStatusCounts.idle,
                })}
              </Badge>
            ) : null}
            {descendantStatusCounts?.paused ? (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t(
                  'sessionHistory.card.descendantPaused',
                  'Paused: {{count}}',
                  {
                    count: descendantStatusCounts.paused,
                  },
                )}
              </Badge>
            ) : null}
            {descendantStatusCounts?.error ? (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t('sessionHistory.card.descendantError', 'Error: {{count}}', {
                  count: descendantStatusCounts.error,
                })}
              </Badge>
            ) : null}
          </div>
        )}

        <div
          className="flex items-center gap-2"
          role="group"
          aria-label={t('sessionHistory.actions.groupAria', 'Session actions')}
        >
          <Button
            size="sm"
            variant={isActive ? 'default' : 'outline'}
            onClick={() => onResume(session.id)}
            className="h-8 min-w-[7.5rem] flex-1 justify-center sm:flex-none"
            aria-label={
              isViewOnly
                ? t(
                    'sessionHistory.actions.viewAria',
                    'View session {{name}}',
                    {
                      name: sessionNameFallback,
                    },
                  )
                : isPaused
                  ? t(
                      'sessionHistory.actions.resumeAria',
                      'Resume session {{name}}',
                      { name: sessionNameFallback },
                    )
                  : t(
                      'sessionHistory.actions.continueAria',
                      'Continue session {{name}}',
                      { name: sessionNameFallback },
                    )
            }
          >
            {isViewOnly ? (
              <>
                <Eye className="mr-1 h-3 w-3" aria-hidden="true" />
                {t('sessionHistory.actions.view', 'View')}
              </>
            ) : (
              <>
                <Play className="mr-1 h-3 w-3" aria-hidden="true" />
                {isPaused
                  ? t('sessionHistory.actions.resume', 'Resume')
                  : t('sessionHistory.actions.continue', 'Continue')}
              </>
            )}
          </Button>
          <Button
            size="sm"
            variant={session.isBookmarked ? 'secondary' : 'outline'}
            onClick={() => onToggleBookmark?.(session.id)}
            aria-label={
              session.isBookmarked
                ? t('sessionHistory.actions.unbookmarkAria', 'Remove bookmark')
                : t('sessionHistory.actions.bookmarkAria', 'Bookmark session')
            }
            className={cn(
              'h-8 shrink-0 px-2.5',
              session.isBookmarked &&
                'border-warning/20 bg-warning/10 text-warning-foreground hover:bg-warning/20',
            )}
          >
            {session.isBookmarked ? (
              <BookmarkCheck className="h-4 w-4" aria-hidden="true" />
            ) : (
              <Bookmark className="h-4 w-4" aria-hidden="true" />
            )}
            <span>
              {session.isBookmarked
                ? t('sessionHistory.actions.bookmarked', 'Bookmarked')
                : t('sessionHistory.actions.bookmark', 'Bookmark')}
            </span>
          </Button>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                size="sm"
                variant="ghost"
                onClick={handleDelete}
                disabled={isDeleting}
                className="h-8 w-8 shrink-0"
                aria-label={t(
                  'sessionHistory.actions.deleteAria',
                  'Delete session {{name}}',
                  { name: sessionNameFallback },
                )}
              >
                <Trash2 className="h-3 w-3" aria-hidden="true" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('sessionHistory.actions.deleteTooltip', 'Delete session')}
            </TooltipContent>
          </Tooltip>
        </div>
      </div>

      {showConfirm && (
        <div className="absolute inset-0 z-10 flex items-end rounded-xl bg-background/95 p-3 backdrop-blur-sm">
          <div className="w-full rounded-lg border bg-card p-3 shadow-lg">
            <div className="mb-2 text-sm font-medium text-foreground">
              {t('sessionHistory.actions.confirmDelete', 'Confirm Delete')}
            </div>
            {descendantCount > 0 ? (
              <div className="flex flex-col gap-2 sm:flex-row">
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <Button
                    size="sm"
                    variant="destructive"
                    onClick={handleDelete}
                    disabled={isDeleting}
                    className="w-full"
                    aria-busy={isDeleting}
                    aria-label={t(
                      'sessionHistory.actions.deleteAllAria',
                      'Delete this session and all subagent sessions',
                    )}
                  >
                    {isDeleting
                      ? t('sessionHistory.actions.deleting', 'Deleting...')
                      : t('sessionHistory.actions.deleteAll', 'Delete all')}
                  </Button>
                  <p className="text-center text-xs text-destructive">
                    {t(
                      'sessionHistory.card.subagentsCount',
                      '+{{count}} subagent',
                      {
                        count: descendantCount,
                      },
                    )}
                  </p>
                </div>
                {onDeleteOnly && (
                  <div className="flex min-w-0 flex-1 flex-col gap-1">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={handleDeleteOnly}
                      disabled={isDeleting}
                      className="w-full"
                      aria-label={t(
                        'sessionHistory.actions.deleteOnlyThisAria',
                        'Delete only this session',
                      )}
                    >
                      {t(
                        'sessionHistory.actions.deleteOnlyThis',
                        'Delete only this',
                      )}
                    </Button>
                    <p className="text-center text-xs text-muted-foreground">
                      {t('sessionHistory.card.subagentsKept', 'Subagents kept')}
                    </p>
                  </div>
                )}
              </div>
            ) : (
              <Button
                size="sm"
                variant="destructive"
                onClick={handleDelete}
                disabled={isDeleting}
                className="w-full"
                aria-busy={isDeleting}
                aria-label={t(
                  'sessionHistory.actions.confirmDeleteAria',
                  'Confirm deletion',
                )}
              >
                {isDeleting
                  ? t('sessionHistory.actions.deleting', 'Deleting...')
                  : t('sessionHistory.actions.confirmDelete', 'Confirm Delete')}
              </Button>
            )}
            <Button
              size="sm"
              variant="outline"
              onClick={handleCancelDelete}
              disabled={isDeleting}
              className="mt-2 w-full"
              aria-label={t(
                'sessionHistory.actions.cancelDeletionAria',
                'Cancel deletion',
              )}
            >
              {t('sessionHistory.actions.cancel', 'Cancel')}
            </Button>
          </div>
        </div>
      )}
    </article>
  );
}
