import { useMemo, useState } from 'react';
import SessionItem from './SessionItem';
import { useSidebar } from '../../components/ui/sidebar';
import { Input } from '@/components/ui';
import { Session } from '@/models/chat';

interface SessionListProps {
  sessions: Session[];
  showSearch?: boolean;
  className?: string;
  emptyMessage?: string;
}

export default function SessionList({
  sessions,
  showSearch = false,
  className = '',
  emptyMessage = 'No sessions found',
}: SessionListProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const { state } = useSidebar();
  const isCollapsed = state === 'collapsed';

  // Filter sessions based on search query
  const filteredSessions = useMemo(() => {
    if (!searchQuery.trim()) {
      return sessions;
    }

    const query = searchQuery.toLowerCase();
    return sessions.filter((session) => {
      const name = session.name?.toLowerCase() || '';
      const description = session.description?.toLowerCase() || '';
      const assistantNames = session.assistants
        .map((a) => a.name.toLowerCase())
        .join(' ');

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
            className="w-full text-muted-foreground placeholder:text-muted-foreground"
          />
        </div>
      )}

      <div className="space-y-1 flex-1">
        {filteredSessions.length === 0
          ? !isCollapsed && (
              <div className="text-center text-muted-foreground py-4 text-sm">
                {searchQuery ? 'No matching sessions' : emptyMessage}
              </div>
            )
          : filteredSessions.map((session) => (
              <SessionItem key={session.id} session={session} />
            ))}
      </div>
    </div>
  );
}
