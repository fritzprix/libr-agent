import { useMemo, useState } from 'react';
import SessionItem from './SessionItem';
import { Input, Badge } from '@/components/ui';
import type { SessionWithHits } from '@/models/search';

interface SessionListProps {
  sessions: SessionWithHits[];
  showSearch?: boolean;
  className?: string;
  emptyMessage?: string;
  isCollapsed?: boolean;
}

export default function SessionList({
  sessions,
  showSearch = false,
  className = '',
  emptyMessage = 'No sessions found',
  isCollapsed = false,
}: SessionListProps) {
  const [searchQuery, setSearchQuery] = useState('');

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
              <div key={session.id} className="relative">
                <SessionItem session={session} isCollapsed={isCollapsed} />
                {/* Display search hit count badge if available */}
                {session.searchHits !== undefined && session.searchHits > 0 && (
                  <div className="absolute top-2 right-2">
                    <Badge variant="secondary" className="text-xs">
                      {session.searchHits}{' '}
                      {session.searchHits === 1 ? 'hit' : 'hits'}
                    </Badge>
                  </div>
                )}
              </div>
            ))}
      </div>
    </div>
  );
}
