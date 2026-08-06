import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import { Eye, RefreshCw, Square } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { isActiveProcessStatus, type ProcessEntry } from './types';

interface ProcessListItemProps {
  process: ProcessEntry;
  isBusy: boolean;
  onReadOutput: (process: ProcessEntry) => void;
  onPoll: (process: ProcessEntry) => void;
  onStop: (process: ProcessEntry) => void;
}

function statusBadgeClass(status: ProcessEntry['status']): string {
  switch (status) {
    case 'starting':
    case 'running':
      return 'border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300';
    case 'finished':
      return 'border-border/60 bg-muted/40 text-muted-foreground';
    case 'failed':
      return 'border-destructive/40 bg-destructive/10 text-destructive';
    case 'killed':
      return 'border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300';
    default:
      return 'border-border/60 bg-muted/40 text-muted-foreground';
  }
}

function formatStartedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function truncateCommand(command: string, max = 96): string {
  if (command.length <= max) {
    return command;
  }
  return `${command.slice(0, max - 1)}…`;
}

export function ProcessListItem({
  process,
  isBusy,
  onReadOutput,
  onPoll,
  onStop,
}: ProcessListItemProps) {
  const { t } = useTranslation();
  const active = isActiveProcessStatus(process.status);
  const title = process.name?.trim() || process.process_id;

  return (
    <div className="border-b border-border/25 px-3 py-3 last:border-b-0">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1 space-y-1.5">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="truncate text-sm font-medium text-foreground">
              {title}
            </span>
            <Badge
              variant="outline"
              className={cn(
                'rounded-sm px-1.5 py-0 text-[10px] uppercase tracking-wide',
                statusBadgeClass(process.status),
              )}
            >
              {t(`agent.processes.status.${process.status}`, process.status)}
            </Badge>
          </div>

          <p
            className="break-all font-mono text-[11px] leading-relaxed text-muted-foreground"
            title={process.command}
          >
            {truncateCommand(process.command)}
          </p>

          <div className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-muted-foreground/90">
            <span className="font-mono">{process.process_id}</span>
            {typeof process.pid === 'number' ? (
              <span>PID {process.pid}</span>
            ) : null}
            <span>{formatStartedAt(process.started_at)}</span>
            {typeof process.exit_code === 'number' ? (
              <span>
                {t('agent.processes.exitCode', 'exit {{code}}', {
                  code: process.exit_code,
                })}
              </span>
            ) : null}
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-0.5">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                disabled={isBusy}
                onClick={() => onReadOutput(process)}
                aria-label={t(
                  'agent.processes.readOutputAria',
                  'Read process output',
                )}
              >
                <Eye className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.processes.readOutput', 'View output')}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                disabled={isBusy}
                onClick={() => onPoll(process)}
                aria-label={t(
                  'agent.processes.pollAria',
                  'Refresh process status',
                )}
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.processes.poll', 'Check status')}
            </TooltipContent>
          </Tooltip>

          {active ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 text-destructive hover:text-destructive"
                  disabled={isBusy}
                  onClick={() => onStop(process)}
                  aria-label={t('agent.processes.stopAria', 'Stop process')}
                >
                  <Square className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t('agent.processes.stop', 'Stop')}
              </TooltipContent>
            </Tooltip>
          ) : null}
        </div>
      </div>
    </div>
  );
}
