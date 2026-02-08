import { Button } from '@/components/ui/button';
import { Trash2, Play, Eye, Circle, Pause, XCircle } from 'lucide-react';
import { useState, useCallback } from 'react';
import { getLogger } from '@/lib/logger';
import { formatRelativeTime } from '@/lib/date-utils';
import type { AgentSession } from '@/models/agent';

const logger = getLogger('SessionCard');

interface SessionCardProps {
  session: AgentSession;
  onResume: (sessionId: string) => void;
  onDelete: (sessionId: string) => void;
}

/**
 * SessionCard Component
 *
 * Displays agent session metadata with action buttons.
 * Used in AgentChatStartView's session history panel.
 */
export function SessionCard({ session, onResume, onDelete }: SessionCardProps) {
  const [isDeleting, setIsDeleting] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);

  const getStatusConfig = useCallback((status: string) => {
    switch (status) {
      case 'busy':
        return {
          icon: 'active',
          badge: 'Active',
          color: 'bg-warning/20 text-warning-foreground',
        };
      case 'idle':
        return {
          icon: 'idle',
          badge: 'Idle',
          color: 'bg-secondary text-secondary-foreground',
        };
      case 'paused':
        return {
          icon: 'paused',
          badge: 'Paused',
          color: 'bg-secondary text-secondary-foreground',
        };
      case 'error':
        return {
          icon: 'error',
          badge: 'Error',
          color: 'bg-destructive/20 text-destructive',
        };
      default:
        return {
          icon: 'unknown',
          badge: 'Unknown',
          color: 'bg-muted text-muted-foreground',
        };
    }
  }, []);

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

  const handleCancelDelete = useCallback(() => {
    setShowConfirm(false);
  }, []);

  const statusConfig = getStatusConfig(session.status);
  const isActive = session.status === 'busy' || session.status === 'idle';
  const isViewOnly = session.status === 'error';

  return (
    <article
      className="border rounded-lg p-4 hover:bg-muted/50 transition-colors"
      aria-label={`Session: ${session.name || session.id.slice(0, 8)}`}
    >
      <div className="flex items-start justify-between mb-2">
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold truncate">
            {session.name || `Session ${session.id.slice(0, 8)}`}
          </h3>
          {session.assistant?.name && (
            <p className="text-xs text-muted-foreground mt-0.5">
              {session.assistant.name}
            </p>
          )}
          <div
            className={`text-xs px-2 py-0.5 rounded-full inline-flex items-center gap-1 mt-1 ${statusConfig.color}`}
            role="status"
            aria-label={`Session status: ${statusConfig.badge}`}
          >
            {statusConfig.icon === 'active' && (
              <Circle className="w-2 h-2 fill-current" />
            )}
            {statusConfig.icon === 'idle' && (
              <Circle className="w-2 h-2 fill-current" />
            )}
            {statusConfig.icon === 'paused' && <Pause className="w-3 h-3" />}
            {statusConfig.icon === 'error' && <XCircle className="w-3 h-3" />}
            {statusConfig.icon === 'unknown' && (
              <Circle className="w-2 h-2 fill-current" />
            )}
            <span>{statusConfig.badge}</span>
          </div>
        </div>
      </div>

      <div className="text-xs text-muted-foreground space-y-1">
        {session.model && session.provider && (
          <div className="flex items-center gap-1">
            <span className="font-medium">Model:</span>
            <span>
              {session.provider}/{session.model}
            </span>
          </div>
        )}
        <div>
          Created{' '}
          {formatRelativeTime(session.createdAt, new Date()) || 'just now'}
        </div>
        {session.updatedAt && (
          <div>
            Updated{' '}
            {formatRelativeTime(session.updatedAt, new Date()) || 'just now'}
          </div>
        )}
      </div>

      <div
        className="flex gap-2 mt-3"
        role="group"
        aria-label="Session actions"
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
                  ? `View session ${session.name || session.id.slice(0, 8)}`
                  : `Continue session ${session.name || session.id.slice(0, 8)}`
              }
            >
              {isViewOnly ? (
                <>
                  <Eye className="w-3 h-3 mr-1" aria-hidden="true" />
                  View
                </>
              ) : (
                <>
                  <Play className="w-3 h-3 mr-1" aria-hidden="true" />
                  Continue
                </>
              )}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={handleDelete}
              disabled={isDeleting}
              aria-label={`Delete session ${session.name || session.id.slice(0, 8)}`}
            >
              <Trash2 className="w-3 h-3" aria-hidden="true" />
            </Button>
          </>
        ) : (
          <>
            <Button
              size="sm"
              variant="destructive"
              onClick={handleDelete}
              disabled={isDeleting}
              className="flex-1"
              aria-busy={isDeleting}
              aria-label="Confirm deletion"
            >
              {isDeleting ? 'Deleting...' : 'Confirm Delete'}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={handleCancelDelete}
              disabled={isDeleting}
              aria-label="Cancel deletion"
            >
              Cancel
            </Button>
          </>
        )}
      </div>
    </article>
  );
}
