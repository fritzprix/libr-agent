import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { PlaybookCard } from './Card';
import type { PlaybookWithMeta } from './grouping-utils';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';

interface PlaybookGroupProps {
  title: string;
  playbooks: PlaybookWithMeta[];
  assistantMap: Record<string, { name: string }>;
  onBookmarkToggle: (id: string, isBookmarked: boolean) => void;
  onDelete: (id: string) => void;
  defaultCollapsed?: boolean;
}

export function PlaybookGroup({
  title,
  playbooks,
  assistantMap,
  onBookmarkToggle,
  onDelete,
  defaultCollapsed = false,
}: PlaybookGroupProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);

  return (
    <div className="space-y-4">
      <div
        className="flex items-center gap-2 cursor-pointer group"
        onClick={() => setIsCollapsed(!isCollapsed)}
      >
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 hover:bg-muted/50"
        >
          {isCollapsed ? (
            <ChevronRight className="h-4 w-4" />
          ) : (
            <ChevronDown className="h-4 w-4" />
          )}
        </Button>
        <h3 className="font-semibold text-lg tracking-tight group-hover:text-primary transition-colors select-none">
          {title}
        </h3>
        <Badge variant="secondary" className="ml-2 font-mono text-xs">
          {playbooks.length}
        </Badge>
        <div className="flex-1 h-px bg-border group-hover:bg-primary/20 transition-colors ml-2" />
      </div>

      <div
        className={cn(
          'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4',
          isCollapsed && 'hidden',
        )}
      >
        {playbooks.map((playbook) => (
          <PlaybookCard
            key={playbook.id}
            playbook={playbook}
            assistantName={assistantMap[playbook.agentId]?.name || 'Unknown'}
            onBookmarkToggle={onBookmarkToggle}
            onDelete={onDelete}
          />
        ))}
      </div>
    </div>
  );
}
