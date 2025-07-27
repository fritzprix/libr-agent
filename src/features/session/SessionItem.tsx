import React, { useCallback, useMemo } from 'react';
import { useSessionContext } from '../../context/SessionContext';
import { Session } from '@/models/chat';
import { useSidebar } from '@/components/ui/sidebar';
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui';

interface SessionItemProps {
  session: Session;
  className?: string;
}

export default function SessionItem({ session }: SessionItemProps) {
  const { state } = useSidebar();
  const isCollapsed = state === 'collapsed';
  const { current, select, delete: onDelete } = useSessionContext();
  const handleSelect = useCallback(() => {
    select(session.id);
  }, [select, session.id]);
  const isSelected = useMemo(
    () => current?.id === session.id,
    [current, session],
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (
        window.confirm(
          `Are you sure you want to delete session "${session.name || 'Untitled Session'}"?`,
        )
      ) {
        onDelete(session.id);
      }
    },
    [onDelete, session.id, session.name],
  );

  const displayName =
    session.name || session.assistants[0]?.name || 'Untitled Session';
  const sessionIcon = session.type === 'single' ? '💬' : '👥';

  return (
    <div
      className="flex items-center group hover:bg-gray-700 rounded-md transition-colors w-full min-w-0 px-1"
      style={{ maxWidth: '100%' }}
    >
      <div className="flex flex-1 min-w-0">
        <Button
          variant="ghost"
          className={`flex-1 min-w-0 justify-start text-left transition-colors duration-150 ${isSelected ? 'text-primary' : 'text-gray-400 hover:text-gray-300'} w-full`}
          onClick={handleSelect}
        >
          {isCollapsed ? (
            sessionIcon
          ) : (
            <span
              className="truncate text-ellipsis overflow-hidden block max-w-full"
              title={displayName}
            >
              {displayName}
            </span>
          )}
        </Button>
      </div>
      {!isCollapsed && (
        <div className="flex-shrink-0 ml-1">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <span>⋮</span>
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
