import { useMemo, useState } from "react";
import { Session } from "../types/chat";
import { Input } from "./ui";
import SessionItem from "./SessionItem";
import { useSidebar } from "./ui/sidebar";

interface SessionListProps {
  sessions: Session[];
  currentSessionId?: string;
  onSelectSession: (sessionId: string) => void;
  onDeleteSession: (sessionId: string) => void;
  showSearch?: boolean;
  className?: string;
  emptyMessage?: string;
}

export default function SessionList({
  sessions,
  currentSessionId,
  onSelectSession,
  onDeleteSession,
  showSearch = false,
  className = "",
  emptyMessage = "No sessions found",
}: SessionListProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const { state } = useSidebar();
  const isCollapsed = state === "collapsed";

  // Filter sessions based on search query
  const filteredSessions = useMemo(() => {
    if (!searchQuery.trim()) {
      return sessions;
    }

    const query = searchQuery.toLowerCase();
    return sessions.filter((session) => {
      const name = session.name?.toLowerCase() || "";
      const description = session.description?.toLowerCase() || "";
      const assistantNames = session.assistants
        .map((a) => a.name.toLowerCase())
        .join(" ");

      return (
        name.includes(query) ||
        description.includes(query) ||
        assistantNames.includes(query)
      );
    });
  }, [sessions, searchQuery]);

  return (
    <div className={`flex flex-col ${className}`}>
      {showSearch && !isCollapsed && (
        <div className="mb-4">
          <Input
            placeholder="Search sessions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-gray-800 border-gray-600 text-gray-300 placeholder-gray-500"
          />
        </div>
      )}

      <div className="space-y-1 flex-1 overflow-y-auto">
        {filteredSessions.length === 0
          ? !isCollapsed && (
              <div className="text-center text-gray-500 py-4 text-sm">
                {searchQuery ? "No matching sessions" : emptyMessage}
              </div>
            )
          : filteredSessions.map((session) => (
              <SessionItem
                key={session.id}
                session={session}
                isSelected={session.id === currentSessionId}
                onSelect={onSelectSession}
                onDelete={onDeleteSession}
              />
            ))}
      </div>
    </div>
  );
}
