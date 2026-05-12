import { useState, useId } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { PlaybookCard } from './Card';
import type { PlaybookWithMeta } from './types';
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
  const { t } = useTranslation();
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);
  const panelId = useId();

  return (
    <div className="space-y-4">
      <h3 className="w-full">
        <button
          type="button"
          aria-expanded={!isCollapsed}
          aria-controls={panelId}
          aria-label={t('playbook.group.ariaLabel', {
            title,
            defaultValue: `${title} ${t('playbook.title')}`,
          })}
          className="flex w-full items-center gap-2 cursor-pointer group focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-md"
          onClick={() => setIsCollapsed(!isCollapsed)}
        >
          <span className="flex h-6 w-6 items-center justify-center rounded-md hover:bg-muted/50 text-muted-foreground group-hover:text-foreground transition-colors">
            {isCollapsed ? (
              <ChevronRight className="h-4 w-4" />
            ) : (
              <ChevronDown className="h-4 w-4" />
            )}
          </span>
          <span className="font-semibold text-lg tracking-tight group-hover:text-primary transition-colors select-none">
            {title}
          </span>
          <Badge variant="secondary" className="ml-2 font-mono text-xs">
            {playbooks.length}
          </Badge>
          <span className="flex-1 h-px bg-border group-hover:bg-primary/20 transition-colors ml-2" />
        </button>
      </h3>

      <div
        id={panelId}
        className={cn(
          'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4',
          isCollapsed && 'hidden',
        )}
      >
        {playbooks.map((playbook) => (
          <PlaybookCard
            key={playbook.id}
            playbook={playbook}
            assistantName={
              assistantMap[playbook.agentId]?.name ||
              t('playbook.card.unknownAssistant')
            }
            onBookmarkToggle={onBookmarkToggle}
            onDelete={onDelete}
          />
        ))}
      </div>
    </div>
  );
}
