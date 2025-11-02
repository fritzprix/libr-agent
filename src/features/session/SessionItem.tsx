import React, { useCallback, useMemo } from 'react';
import { MessageCircle, Users } from 'lucide-react';
import { useSessionContext } from '../../context/SessionContext';
import { Session } from '@/models/chat';
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui';
import { getLogger } from '@/lib/logger';
import { confirm } from '@tauri-apps/plugin-dialog';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

const logger = getLogger('SessionItem');

interface SessionItemProps {
  session: Session;
  className?: string;
  isCollapsed?: boolean;
}

function formatRelativeTime(target: Date, reference: Date): string | null {
  const diffMs = target.getTime() - reference.getTime();
  const diffSeconds = Math.round(diffMs / 1000);

  const thresholds = [
    { limit: 60, divisor: 1, unit: 'second' as const },
    { limit: 3600, divisor: 60, unit: 'minute' as const },
    { limit: 86400, divisor: 3600, unit: 'hour' as const },
    { limit: 604800, divisor: 86400, unit: 'day' as const },
    { limit: 2629800, divisor: 604800, unit: 'week' as const },
    { limit: 31557600, divisor: 2629800, unit: 'month' as const },
  ];

  const absSeconds = Math.abs(diffSeconds);

  for (const threshold of thresholds) {
    if (absSeconds < threshold.limit) {
      const value = Math.round(diffSeconds / threshold.divisor);
      return new Intl.RelativeTimeFormat(undefined, {
        numeric: 'auto',
      }).format(value, threshold.unit);
    }
  }

  const years = Math.round(diffSeconds / 31557600);
  return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(
    years,
    'year',
  );
}

function formatSessionTimestamp(dateInput: Date | string | undefined) {
  if (!dateInput) {
    return {
      display: 'Unknown date',
      tooltip: 'Unknown date',
      relative: null as string | null,
    };
  }

  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (Number.isNaN(date.getTime())) {
    return {
      display: 'Invalid date',
      tooltip: 'Invalid date',
      relative: null as string | null,
    };
  }

  const absolute = date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
  const relative = formatRelativeTime(date, new Date());

  const display = relative ? `${absolute} · ${relative}` : absolute;

  return {
    display,
    tooltip: date.toLocaleString(),
    relative,
  };
}

export default function SessionItem({
  session,
  className,
  isCollapsed = false,
}: SessionItemProps) {
  const { current, select, delete: onDelete } = useSessionContext();
  const handleSelect = useCallback(() => {
    select(session.id);
  }, [select, session.id]);
  const isSelected = useMemo(
    () => current?.id === session.id,
    [current, session],
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      logger.info('Attempting to delete session', { sessionId: session.id });

      const userConfirmed = await confirm(
        `Are you sure you want to delete session "${session.name || 'Untitled Session'}"?`,
        { title: 'Confirm Deletion' },
      );

      if (userConfirmed) {
        onDelete(session.id);
      }
    },
    [onDelete, session.id, session.name],
  );

  const displayName =
    session.name || session.assistants[0]?.name || 'Untitled Session';
  const sessionIconComponent =
    session.type === 'single' ? (
      <MessageCircle size={16} />
    ) : (
      <Users size={16} />
    );
  const assistantSummary = useMemo(() => {
    if (!session.assistants?.length) {
      return '';
    }
    return session.assistants.map((assistant) => assistant.name).join(', ');
  }, [session.assistants]);

  const timestampInfo = useMemo(
    () => formatSessionTimestamp(session.createdAt),
    [session.createdAt],
  );

  return (
    <div
      className={cn(
        'flex items-center rounded-lg transition-colors w-full min-w-0 px-2 py-1.5',
        'hover:bg-muted/60',
        className,
      )}
      style={{ maxWidth: '100%' }}
    >
      <div className="flex flex-1 min-w-0">
        <Button
          variant="ghost"
          className={cn(
            'flex-1 min-w-0 justify-start text-left transition-colors duration-150 w-full px-0',
            isSelected
              ? 'text-primary hover:text-primary'
              : 'text-muted-foreground hover:text-foreground hover:no-underline',
          )}
          onClick={handleSelect}
        >
          {isCollapsed ? (
            <span aria-hidden className="text-lg">
              {sessionIconComponent}
            </span>
          ) : (
            <div className="flex w-full flex-col gap-1.5 min-w-0 text-left">
              <div className="flex items-center gap-2 min-w-0">
                <span aria-hidden className="text-base text-muted-foreground">
                  {sessionIconComponent}
                </span>
                <span
                  className="truncate font-medium text-foreground"
                  title={displayName}
                >
                  {displayName}
                </span>
              </div>
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                {assistantSummary && (
                  <span className="truncate" title={assistantSummary}>
                    {assistantSummary}
                  </span>
                )}
                {timestampInfo.display && (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="whitespace-nowrap">
                        {timestampInfo.display}
                      </span>
                    </TooltipTrigger>
                    <TooltipContent sideOffset={6}>
                      {timestampInfo.tooltip}
                    </TooltipContent>
                  </Tooltip>
                )}
              </div>
            </div>
          )}
        </Button>
      </div>
      {!isCollapsed && (
        <div className="flex-shrink-0 ml-2 text-muted-foreground">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                className="h-8 w-8 inline-flex items-center justify-center rounded-md hover:bg-muted"
                aria-label="Session options"
              >
                ⋮
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent sideOffset={5} align="end">
              <DropdownMenuItem onClick={handleDelete}>Delete</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}
    </div>
  );
}
