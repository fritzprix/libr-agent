import { Button } from '@/components/ui/button';
import { Trash2, Play, Eye } from 'lucide-react';
import { useState, useCallback } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('SessionCard');

// Simple relative time formatter (avoids external dependency)
function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return 'just now';
  if (diffMins < 60) return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
  if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
  if (diffDays < 30) return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;

  const diffMonths = Math.floor(diffDays / 30);
  if (diffMonths < 12)
    return `${diffMonths} month${diffMonths > 1 ? 's' : ''} ago`;

  const diffYears = Math.floor(diffMonths / 12);
  return `${diffYears} year${diffYears > 1 ? 's' : ''} ago`;
}

interface AgentSessionMetadata {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  createdAt: Date;
  updatedAt?: Date;
}

interface SessionCardProps {
  session: AgentSessionMetadata;
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
          badge: '🟢 Active',
          color: 'bg-green-500/20 text-green-700 dark:text-green-400',
        };
      case 'idle':
        return {
          badge: '🔵 Idle',
          color: 'bg-blue-500/20 text-blue-700 dark:text-blue-400',
        };
      case 'paused':
        return {
          badge: '⏸️ Paused',
          color: 'bg-yellow-500/20 text-yellow-700 dark:text-yellow-400',
        };
      case 'error':
        return {
          badge: '🔴 Error',
          color: 'bg-red-500/20 text-red-700 dark:text-red-400',
        };
      default:
        return {
          badge: '⚪ Unknown',
          color: 'bg-gray-500/20 text-gray-700 dark:text-gray-400',
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
          <div
            className={`text-xs px-2 py-0.5 rounded-full inline-block mt-1 ${statusConfig.color}`}
            role="status"
            aria-label={`Session status: ${statusConfig.badge}`}
          >
            {statusConfig.badge}
          </div>
        </div>
      </div>

      <div className="text-xs text-muted-foreground space-y-1">
        <div>Created {formatRelativeTime(session.createdAt)}</div>
        {session.updatedAt && (
          <div>Updated {formatRelativeTime(session.updatedAt)}</div>
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
