import { useCallback } from "react";
import { Session } from "../types/chat";
import { Button } from "./ui";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "./ui/dropdown-menu";
import { useSidebar } from "./ui/sidebar";

interface SessionItemProps {
  session: Session;
  isSelected?: boolean;
  onSelect: (sessionId: string) => void;
  onDelete: (sessionId: string) => void;
  className?: string;
}

export default function SessionItem({
  session,
  isSelected = false,
  onSelect,
  onDelete,
  className = "",
}: SessionItemProps) {
  const { state } = useSidebar();
  const isCollapsed = state === "collapsed";
  const handleSelect = useCallback(() => {
    onSelect(session.id);
  }, [onSelect, session.id]);

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (
        window.confirm(
          `Are you sure you want to delete session "${session.name || "Untitled Session"}"?`,
        )
      ) {
        onDelete(session.id);
      }
    },
    [onDelete, session.id, session.name],
  );

  const displayName =
    session.name || session.assistants[0]?.name || "Untitled Session";
  const sessionIcon = session.type === "single" ? "💬" : "👥";

  return (
    <div
      className={`flex items-center justify-between group hover:bg-gray-700 rounded-md transition-colors ${
        isSelected ? "bg-green-900/20 text-green-400" : "text-gray-400"
      } ${className}`}
    >
      <Button
        variant="ghost"
        className={`flex-1 justify-start text-left transition-colors duration-150 ${
          isSelected ? "text-green-400" : "text-gray-400 hover:text-gray-300"
        }`}
        onClick={handleSelect}
      >
        {isCollapsed ? (
          sessionIcon
        ) : (
          <span className="truncate" title={displayName}>
            {displayName}
          </span>
        )}
      </Button>

      {!isCollapsed && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm">
              <span>⋮</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={handleDelete}>Delete</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}
