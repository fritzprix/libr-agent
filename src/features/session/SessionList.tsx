import { memo } from 'react';
import { useParams } from 'react-router-dom';
import SessionItem from './SessionItem';
import { Badge } from '@/components/ui';
import type { AgentSession } from '@/models/agent';

interface SessionListProps {
  sessions: AgentSession[];
  searchHits?: Map<string, number>;
  className?: string;
  emptyMessage?: string;
  isCollapsed?: boolean;
}

function SessionList({
  sessions,
  searchHits,
  className = '',
  emptyMessage = 'No sessions found',
  isCollapsed = false,
}: SessionListProps) {
  const { sessionId } = useParams();

  return (
    <div className={`flex flex-col ${className}`}>
      <div className="space-y-1 flex-1">
        {sessions.length === 0
          ? !isCollapsed && (
              <div className="text-center text-muted-foreground py-4 text-sm">
                {emptyMessage}
              </div>
            )
          : sessions.map((session) => {
              const hits = searchHits?.get(session.id);
              return (
                <div key={session.id} className="relative">
                  <SessionItem
                    session={session}
                    isCollapsed={isCollapsed}
                    isActive={session.id === sessionId}
                  />
                  {/* Display search hit count badge if available */}
                  {hits !== undefined && hits > 0 && (
                    <div className="absolute top-2 right-2">
                      <Badge variant="secondary" className="text-xs">
                        {hits} {hits === 1 ? 'hit' : 'hits'}
                      </Badge>
                    </div>
                  )}
                </div>
              );
            })}
      </div>
    </div>
  );
}

export default memo(SessionList);
