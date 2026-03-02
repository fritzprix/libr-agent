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
} from 'lucide-react';
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { getLogger } from '@/lib/logger';
import { formatRelativeTime } from '@/lib/date-utils';
import type { AgentSession } from '@/models/agent';

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
  /** Delete only this session, promoting children to top-level (SP7). */
  onDeleteOnly?: (sessionId: string) => void;
  /** Toggle bookmark on this session (SP12). */
  onToggleBookmark?: (sessionId: string) => void;
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
  onDeleteOnly,
  onToggleBookmark,
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
          };
        case 'idle':
          return {
            icon: 'idle',
            badge: t('sessionHistory.status.idle', 'Idle'),
            variant: 'secondary' as const,
            className: '',
          };
        case 'paused':
          return {
            icon: 'paused',
            badge: t('sessionHistory.status.paused', 'Paused'),
            variant: 'secondary' as const,
            className: 'opacity-75',
          };
        case 'error':
          return {
            icon: 'error',
            badge: t('sessionHistory.status.error', 'Error'),
            variant: 'destructive' as const,
            className:
              'bg-destructive/10 text-destructive border-destructive/20',
          };
        default:
          return {
            icon: 'unknown',
            badge: t('sessionHistory.status.unknown', 'Unknown'),
            variant: 'outline' as const,
            className: 'text-muted-foreground',
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

  return (
    <article
      className="border rounded-xl p-4 hover:bg-muted/50 transition-colors"
      style={{ marginLeft: `${nestingLevel * 16}px` }}
      aria-label={t('sessionHistory.card.ariaLabel', 'Session: {{name}}', {
        name: sessionNameFallback,
      })}
    >
      <div className="flex items-start justify-between mb-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="font-semibold truncate">{sessionNameFallback}</h3>
            <Badge
              variant={statusConfig.variant}
              className={statusConfig.className}
              role="status"
              aria-label={t(
                'sessionHistory.card.statusAriaLabel',
                'Session status: {{status}}',
                { status: statusConfig.badge },
              )}
            >
              {statusConfig.icon === 'active' && (
                <Circle className="w-3 h-3 fill-current" />
              )}
              {statusConfig.icon === 'idle' && (
                <Circle className="w-3 h-3 fill-current" />
              )}
              {statusConfig.icon === 'paused' && <Pause className="w-3 h-3" />}
              {statusConfig.icon === 'error' && <XCircle className="w-3 h-3" />}
              {statusConfig.icon === 'unknown' && (
                <Circle className="w-3 h-3 fill-current" />
              )}
              {statusConfig.badge}
            </Badge>
          </div>
          {session.assistant?.name && (
            <p className="text-xs text-muted-foreground">
              {session.assistant.name}
            </p>
          )}
        </div>
      </div>

      <div className="text-xs text-muted-foreground space-y-1">
        {(depthLabel || shortParentId || shortLineageId) && (
          <div className="flex flex-wrap items-center gap-1">
            <Badge variant="secondary">{relationBadge}</Badge>
            {depthLabel && <Badge variant="outline">{depthLabel}</Badge>}
            {shortParentId && (
              <Badge variant="outline">
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
                onClick={() => onLineageSelect?.(session.lineageId!)}
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
          </div>
        )}
        {lineageHint && <div>{lineageHint}</div>}
        {session.model && session.provider && (
          <div className="flex items-center gap-1">
            <span className="font-medium">
              {t('sessionHistory.card.model', 'Model:')}
            </span>
            <span>
              {session.provider}/{session.model}
            </span>
          </div>
        )}
        <div>
          {t('sessionHistory.card.createdAt', 'Created {{time}}', {
            time:
              formatRelativeTime(session.createdAt, new Date()) ||
              t('sessionHistory.card.justNow', 'just now'),
          })}
        </div>
        {session.updatedAt && (
          <div>
            {t('sessionHistory.card.updatedAt', 'Updated {{time}}', {
              time:
                formatRelativeTime(session.updatedAt, new Date()) ||
                t('sessionHistory.card.justNow', 'just now'),
            })}
          </div>
        )}
      </div>

      <div
        className="flex gap-2 mt-3"
        role="group"
        aria-label={t('sessionHistory.actions.groupAria', 'Session actions')}
      >
        {!showConfirm ? (
          <>
            <Button
              size="sm"
              variant={isActive ? 'default' : 'outline'}
              onClick={() => onResume(session.id)}
              className="flex-1"
              aria-label={
                isViewOnly
                  ? t(
                      'sessionHistory.actions.viewAria',
                      'View session {{name}}',
                      { name: sessionNameFallback },
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
                  <Eye className="w-3 h-3 mr-1" aria-hidden="true" />
                  {t('sessionHistory.actions.view', 'View')}
                </>
              ) : isPaused ? (
                <>
                  <Play className="w-3 h-3 mr-1" aria-hidden="true" />
                  {t('sessionHistory.actions.resume', 'Resume')}
                </>
              ) : (
                <>
                  <Play className="w-3 h-3 mr-1" aria-hidden="true" />
                  {t('sessionHistory.actions.continue', 'Continue')}
                </>
              )}
            </Button>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onToggleBookmark?.(session.id)}
                  aria-label={
                    session.isBookmarked
                      ? t(
                          'sessionHistory.actions.unbookmarkAria',
                          'Remove bookmark',
                        )
                      : t(
                          'sessionHistory.actions.bookmarkAria',
                          'Bookmark session',
                        )
                  }
                  className={session.isBookmarked ? 'text-yellow-500' : ''}
                >
                  {session.isBookmarked ? (
                    <BookmarkCheck className="w-3 h-3" aria-hidden="true" />
                  ) : (
                    <Bookmark className="w-3 h-3" aria-hidden="true" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {session.isBookmarked
                  ? t('sessionHistory.actions.unbookmark', 'Remove bookmark')
                  : t('sessionHistory.actions.bookmark', 'Bookmark')}
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={handleDelete}
                  disabled={isDeleting}
                  aria-label={t(
                    'sessionHistory.actions.deleteAria',
                    'Delete session {{name}}',
                    { name: sessionNameFallback },
                  )}
                >
                  <Trash2 className="w-3 h-3" aria-hidden="true" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t('sessionHistory.actions.deleteTooltip', 'Delete session')}
              </TooltipContent>
            </Tooltip>
          </>
        ) : (
          <>
            {descendantCount > 0 ? (
              <>
                <div className="flex w-full gap-2">
                  <div className="flex flex-col flex-1 gap-0.5">
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
                    <p className="text-xs text-destructive text-center">
                      {t(
                        'sessionHistory.card.subagentsCount',
                        '+{{count}} subagent',
                        { count: descendantCount },
                      )}
                    </p>
                  </div>
                  {onDeleteOnly && (
                    <div className="flex flex-col flex-1 gap-0.5">
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
                      <p className="text-xs text-muted-foreground text-center">
                        {t(
                          'sessionHistory.card.subagentsKept',
                          'Subagents kept',
                        )}
                      </p>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <Button
                size="sm"
                variant="destructive"
                onClick={handleDelete}
                disabled={isDeleting}
                className="flex-1"
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
              aria-label={t(
                'sessionHistory.actions.cancelDeletionAria',
                'Cancel deletion',
              )}
            >
              {t('sessionHistory.actions.cancel', 'Cancel')}
            </Button>
          </>
        )}
      </div>
    </article>
  );
}
