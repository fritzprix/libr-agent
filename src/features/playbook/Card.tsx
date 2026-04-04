import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Bookmark, Play, Trash2 } from 'lucide-react';
import type { Playbook } from '@/types/playbook';
import { cn } from '@/lib/utils';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { getDateFormatter } from '@/lib/date-utils';

interface PlaybookCardProps {
  playbook: Playbook & { id: string; createdAt: Date };
  assistantName: string;
  onBookmarkToggle: (id: string, isBookmarked: boolean) => void;
  onDelete: (id: string) => void;
  className?: string;
}

export function PlaybookCard({
  playbook,
  assistantName,
  onBookmarkToggle,
  onDelete,
  className,
}: PlaybookCardProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();

  const handleStart = () => {
    navigate(`/agent?playbookId=${playbook.id}`);
  };

  return (
    <Card
      className={cn(
        'group flex flex-col h-full hover:shadow-md transition-shadow',
        className,
      )}
    >
      <CardHeader className="pb-3 space-y-2">
        <div className="flex justify-between items-start gap-4">
          <Badge
            variant="outline"
            className="font-normal text-xs text-muted-foreground w-fit truncate max-w-36"
          >
            {assistantName}
          </Badge>
          <div className="flex items-center gap-1 -mr-2 -mt-2">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className={cn(
                    'h-8 w-8 hover:bg-transparent',
                    playbook.isBookmarked
                      ? 'text-warning hover:text-warning/80'
                      : 'text-muted-foreground/30 hover:text-muted-foreground group-hover:text-muted-foreground',
                  )}
                  onClick={(e) => {
                    e.stopPropagation();
                    onBookmarkToggle(playbook.id, !playbook.isBookmarked);
                  }}
                >
                  <Bookmark
                    className={cn(
                      'h-4 w-4',
                      playbook.isBookmarked && 'fill-current',
                    )}
                  />
                  <span className="sr-only">{t('playbook.card.bookmark')}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t('playbook.card.bookmark')}</TooltipContent>
            </Tooltip>
          </div>
        </div>
        <CardTitle
          className="text-lg leading-tight line-clamp-2"
          title={playbook.goal}
        >
          {playbook.goal}
        </CardTitle>
        <CardDescription className="line-clamp-1 text-xs">
          {t('playbook.card.created', {
            date: getDateFormatter(i18n.language, {
              year: 'numeric',
              month: 'numeric',
              day: 'numeric',
            }).format(playbook.createdAt),
          })}
        </CardDescription>
      </CardHeader>

      <CardContent className="flex-1 pb-3">
        <div className="text-sm text-muted-foreground line-clamp-3 mb-4">
          {/* Display steps summary or description if available */}
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="secondary" className="text-xs font-mono">
              {t('playbook.card.steps', { count: playbook.workflow.length })}
            </Badge>
          </div>
          {playbook.initialCommand && (
            <p className="italic text-xs border-l-2 pl-2 border-border/50 text-muted-foreground/80 truncate">
              &quot;{playbook.initialCommand}&quot;
            </p>
          )}
        </div>
      </CardContent>

      <CardFooter className="pt-0 flex justify-between gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="text-muted-foreground hover:text-destructive h-8 w-8"
              onClick={() => onDelete(playbook.id)}
              aria-label={t('playbook.card.deleteTooltip')}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t('playbook.card.deleteTooltip')}</TooltipContent>
        </Tooltip>

        <Button
          size="sm"
          className="gap-2 w-full max-w-32"
          onClick={handleStart}
        >
          <Play className="h-3.5 w-3.5" />
          {t('playbook.card.start')}
        </Button>
      </CardFooter>
    </Card>
  );
}
