import React, { useCallback, useMemo, memo } from 'react';
import { Bot } from 'lucide-react';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { useNavigate } from 'react-router-dom';
import { AgentSession } from '@/models/agent';
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
  session: AgentSession;
  className?: string;
  isCollapsed?: boolean;
  isActive?: boolean;
}

function SessionItem({
  session,
  className,
  isCollapsed = false,
  isActive = false,
}: SessionItemProps) {
  const { deleteSession } = useAgentSessionListActions();
  const navigate = useNavigate();

  const handleSelect = useCallback(() => {
    navigate(`/agent/${session.id}`);
  }, [navigate, session.id]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      logger.info('Attempting to delete session', { sessionId: session.id });

      const userConfirmed = await confirm(
        `Are you sure you want to delete session "${session.name || 'Untitled Session'}"?`,
        { title: 'Confirm Deletion' },
      );

      if (userConfirmed) {
        await deleteSession(session.id);
        // If deleted current session, navigate away
        if (isActive) {
          navigate('/agent');
        }
      }
    },
    [deleteSession, session.id, session.name, isActive, navigate],
  );

  const displayName =
    session.name || session.assistant?.name || 'Untitled Session';

  const sessionIconComponent = <Bot size={16} />;

  const assistantSummary = session.assistant?.name || '';

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
            isActive
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

export default memo(SessionItem);
