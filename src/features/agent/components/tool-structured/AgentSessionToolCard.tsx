import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle2,
  ChevronDown,
  Ban,
  PauseCircle,
  ExternalLink,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import { cn } from '@/lib/utils';
import type { CheckSessionResult } from './types';

export interface AgentSessionToolCardProps {
  data: CheckSessionResult;
}

function truncatePreview(text: string, maxChars: number): string {
  const trimmed = text.trim();
  if (trimmed.length <= maxChars) return trimmed;
  return `${trimmed.slice(0, maxChars).trimEnd()}…`;
}

function resolveCardKind(
  data: CheckSessionResult,
): 'cancelled' | 'paused' | 'completed' {
  if (data.terminatedByUser || data.responseStatus === 'cancelled') {
    return 'cancelled';
  }
  if (
    data.status.toLowerCase() === 'paused' ||
    data.responseStatus === 'paused'
  ) {
    return 'paused';
  }
  return 'completed';
}

/**
 * Compact structured result card for agent__checkSession.
 * Default collapsed so parent chat is not flooded with child output.
 */
export const AgentSessionToolCard: React.FC<AgentSessionToolCardProps> = ({
  data,
}) => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const kind = resolveCardKind(data);
  const label =
    data.assistantName?.trim() ||
    data.sessionId ||
    t('agent.toolStructured.sessionFallback', 'Subagent');
  const summary = (data.result ?? data.message ?? '').trim();
  const preview = summary
    ? truncatePreview(summary, 160)
    : t('agent.toolStructured.sessionNoSummary', 'No summary available');

  const toneClass =
    kind === 'cancelled'
      ? 'border-destructive/30 bg-destructive/5'
      : kind === 'paused'
        ? 'border-amber-500/30 bg-amber-500/5'
        : 'border-emerald-500/30 bg-emerald-500/5';

  const Icon =
    kind === 'cancelled'
      ? Ban
      : kind === 'paused'
        ? PauseCircle
        : CheckCircle2;

  const iconClass =
    kind === 'cancelled'
      ? 'text-destructive'
      : kind === 'paused'
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-emerald-600 dark:text-emerald-400';

  const title =
    kind === 'cancelled'
      ? t(
          'agent.toolStructured.sessionStoppedTitle',
          '"{{name}}" stopped by user',
          { name: label },
        )
      : kind === 'paused'
        ? t('agent.toolStructured.sessionPausedTitle', '"{{name}}" paused', {
            name: label,
          })
        : t(
            'agent.toolStructured.sessionCompletedTitle',
            '"{{name}}" completed',
            { name: label },
          );

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn('rounded-lg border p-3', toneClass)}
      data-testid="tool-structured-check-session"
    >
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center justify-between gap-2 text-left"
        >
          <div className="flex min-w-0 items-center gap-2">
            <Icon className={cn('h-4 w-4 shrink-0', iconClass)} />
            <span className="truncate text-sm font-medium">{title}</span>
            {typeof data.turnCount === 'number' ? (
              <Badge variant="outline" className="shrink-0 text-[10px]">
                {t('agent.toolStructured.sessionTurns', '{{count}} turns', {
                  count: data.turnCount,
                })}
              </Badge>
            ) : null}
          </div>
          <Badge variant="outline" className="shrink-0 gap-1">
            {open
              ? t('agent.toolStructured.hideResult', 'Hide')
              : t('agent.toolStructured.viewResult', 'View result')}
            <ChevronDown
              className={cn(
                'h-3 w-3 transition-transform',
                open && 'rotate-180',
              )}
            />
          </Badge>
        </button>
      </CollapsibleTrigger>

      {!open ? (
        <p className="mt-2 line-clamp-2 text-xs text-muted-foreground">
          {preview}
        </p>
      ) : null}

      <CollapsibleContent className="mt-2 border-t border-border/60 pt-2">
        <pre className="max-h-48 overflow-y-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">
          {summary || preview}
        </pre>
        <Button
          type="button"
          variant="link"
          size="sm"
          className="mt-1 h-auto px-0 text-xs"
          onClick={() => navigate(`/agent/${data.sessionId}`)}
        >
          {t(
            'agent.toolStructured.openChildSession',
            'Open child session',
          )}
          <ExternalLink className="ml-1 h-3 w-3" />
        </Button>
      </CollapsibleContent>
    </Collapsible>
  );
};
