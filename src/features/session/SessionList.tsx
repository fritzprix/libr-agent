import { useMemo, useState, memo, useEffect, useRef } from 'react';
import { useParams } from 'react-router-dom';
import SessionItem from './SessionItem';
import { Input, Badge } from '@/components/ui';
import { useDebounced } from '@/hooks/useDebounced';
import { filterSessions } from '@/lib/session-utils';
import type { AgentSession } from '@/models/agent';

interface SessionListProps {
  sessions: AgentSession[];
  searchHits?: Map<string, number>;
  showSearch?: boolean;
  className?: string;
  emptyMessage?: string;
  isCollapsed?: boolean;
}

function SessionList({
  sessions,
  searchHits,
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

  // --- Virtualization / Infinite Scroll Optimization ---
  // Only render a subset of sessions initially to improve performance
  const [visibleCount, setVisibleCount] = useState(20);
  const observerTarget = useRef<HTMLDivElement>(null);

  // Reset visible count when search query changes
  useEffect(() => {
    setVisibleCount(20);
  }, [debouncedQuery]);

  // Ensure visibleCount does not exceed the number of filtered sessions
  useEffect(() => {
    if (visibleCount > filteredSessions.length) {
      setVisibleCount(filteredSessions.length);
    }
  }, [filteredSessions.length, visibleCount]);

  const visibleSessions = useMemo(() => {
    return filteredSessions.slice(0, visibleCount);
  }, [filteredSessions, visibleCount]);

  // Intersection Observer to load more items when scrolling to bottom
  useEffect(() => {
    const element = observerTarget.current;
    if (!element) return;

    // Only observe when there are more items to load
    if (visibleCount >= filteredSessions.length) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.length === 0) {
          return;
        }
        const firstEntry = entries[0];
        if (firstEntry.isIntersecting) {
          setVisibleCount((prev) => prev + 20);
        }
      },
      { rootMargin: '200px' } // Pre-load content before user hits the exact bottom
    );

    observer.observe(element);

    return () => observer.disconnect();
  }, [filteredSessions.length]);
  // ---------------------------------------------------

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
        {filteredSessions.length === 0 ? (
          !isCollapsed && (
            <div className="text-center text-muted-foreground py-4 text-sm">
              {searchQuery ? 'No matching sessions' : emptyMessage}
            </div>
          )
        ) : (
          <>
            {visibleSessions.map((session) => {
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

            {/* Infinite Scroll Sentinel */}
            {visibleCount < filteredSessions.length && (
              <div
                ref={observerTarget}
                className="h-4 w-full opacity-0 pointer-events-none"
                aria-hidden="true"
              />
            )}
          </>
        )}
      </div>
    </div>
  );
}

export default memo(SessionList);
