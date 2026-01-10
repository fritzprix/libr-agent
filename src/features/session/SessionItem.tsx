import React, { useCallback, useMemo, memo } from 'react';
import { MessageCircle, Users } from 'lucide-react';
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
import { formatSessionTimestamp } from '@/lib/date-utils';

const logger = getLogger('SessionItem');

interface SessionItemProps {
  session: Session;
  className?: string;
  isCollapsed?: boolean;
  isSelected?: boolean;
  onDelete: (id: string) => Promise<void>;
  onSelect: (id: string) => void;
}

function SessionItemComponent({
  session,
  className,
  isCollapsed = false,
  isSelected = false,
  onDelete,
  onSelect,
}: SessionItemProps) {
  const handleSelect = useCallback(() => {
    onSelect(session.id);
  }, [onSelect, session.id]);

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

const SessionItem = memo(SessionItemComponent);
export default SessionItem;
