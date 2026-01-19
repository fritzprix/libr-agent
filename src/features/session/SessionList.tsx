import { useMemo, useState, memo } from 'react';
import { useParams } from 'react-router-dom';
import SessionItem from './SessionItem';
import { Input, Badge } from '@/components/ui';
import { useDebounced } from '@/hooks/useDebounced';
import { filterSessions } from '@/lib/session-utils';
import type { SessionWithHits } from '@/models/search';

interface SessionListProps {
  sessions: SessionWithHits[];
  showSearch?: boolean;
  className?: string;
  emptyMessage?: string;
  isCollapsed?: boolean;
}

function SessionList({
  sessions,
  showSearch = false,
  className = '',
  emptyMessage = 'No sessions found',
  isCollapsed = false,
}: SessionListProps) {
  const { sessionId } = useParams();
  const [searchQuery, setSearchQuery] = useState('');
  // Debounce search query to reduce filtering operations during typing
  const debouncedQuery = useDebounced(searchQuery, 300);

  // Filter sessions based on debounced search query
  const filteredSessions = useMemo(() => {
    return filterSessions(sessions, debouncedQuery);
  }, [sessions, debouncedQuery]);

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
                <SessionItem
                  session={session}
                  isCollapsed={isCollapsed}
                  isActive={session.id === sessionId}
                />
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

export default memo(SessionList);
