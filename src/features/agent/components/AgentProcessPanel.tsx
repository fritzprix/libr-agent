import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import { AlertTriangle, Loader2, RefreshCw, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ProcessListItem } from './process-panel/ProcessListItem';
import { ProcessOutputDialog } from './process-panel/ProcessOutputDialog';
import { useProcesses } from './process-panel/useProcesses';

interface AgentProcessPanelProps {
  isVisible?: boolean;
  variant?: 'rail' | 'sheet';
}

/**
 * Background process panel MVP (#1686).
 * Lists session processes via workspace__listProcesses and exposes
 * read / poll / stop actions through the existing builtin tool bridge.
 */
export function AgentProcessPanel({
  isVisible = true,
  variant = 'rail',
}: AgentProcessPanelProps) {
  const { t } = useTranslation();
  const {
    processes,
    total,
    running,
    finished,
    loading,
    error,
    actionProcessId,
    outputOpen,
    outputLoading,
    outputResult,
    outputError,
    refresh,
    stopProcess,
    pollProcess,
    readOutput,
    closeOutput,
  } = useProcesses({ enabled: isVisible });

  return (
    <>
      <Card
        id="agent-processes-panel"
        role="region"
        aria-label={t('agent.processes.title', 'Background Processes')}
        aria-hidden={!isVisible}
        className={cn(
          'flex h-full flex-col rounded-none border-0 border-r border-border/60 bg-background/95 shadow-none',
          variant === 'sheet' && 'border-r-0',
          !isVisible && 'pointer-events-none',
        )}
      >
        <CardHeader className="shrink-0 space-y-2 border-b border-border/50 px-4 py-3">
          <div className="flex items-center justify-between gap-2">
            <CardTitle className="text-sm font-medium">
              {t('agent.processes.title', 'Background Processes')}
            </CardTitle>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  disabled={loading}
                  onClick={() => {
                    void refresh();
                  }}
                  aria-label={t(
                    'agent.processes.refreshAria',
                    'Refresh process list',
                  )}
                >
                  {loading ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <RefreshCw className="h-3.5 w-3.5" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t('agent.processes.refresh', 'Refresh')}
              </TooltipContent>
            </Tooltip>
          </div>

          <div className="flex flex-wrap gap-2">
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {t('agent.processes.summaryTotal', '{{count}} total', {
                count: total,
              })}
            </div>
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {t('agent.processes.summaryRunning', '{{count}} running', {
                count: running,
              })}
            </div>
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {t('agent.processes.summaryFinished', '{{count}} finished', {
                count: finished,
              })}
            </div>
          </div>
        </CardHeader>

        <CardContent className="flex min-h-0 flex-1 flex-col p-0">
          {error ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
              <AlertTriangle
                className="h-5 w-5 text-destructive/80"
                aria-hidden="true"
              />
              <p className="max-w-[16rem] text-sm text-muted-foreground">
                {error}
              </p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                  void refresh();
                }}
              >
                {t('agent.processes.retry', 'Retry')}
              </Button>
            </div>
          ) : loading && processes.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-muted-foreground">
              <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
              <p className="text-sm">
                {t('agent.processes.loading', 'Loading processes…')}
              </p>
            </div>
          ) : processes.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
              <Terminal
                className="h-5 w-5 text-muted-foreground/60"
                aria-hidden="true"
              />
              <p className="max-w-[16rem] text-sm text-muted-foreground">
                {t(
                  'agent.processes.empty',
                  'No background processes in this session yet.',
                )}
              </p>
            </div>
          ) : (
            <div className="min-h-0 flex-1 overflow-hidden">
              <ScrollArea className="h-full">
                <div>
                  {processes.map((process) => (
                    <ProcessListItem
                      key={process.process_id}
                      process={process}
                      isBusy={actionProcessId === process.process_id}
                      onReadOutput={(item) => {
                        void readOutput(item);
                      }}
                      onPoll={(item) => {
                        void pollProcess(item);
                      }}
                      onStop={(item) => {
                        void stopProcess(item);
                      }}
                    />
                  ))}
                </div>
              </ScrollArea>
            </div>
          )}
        </CardContent>
      </Card>

      <ProcessOutputDialog
        open={outputOpen}
        loading={outputLoading}
        result={outputResult}
        error={outputError}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            closeOutput();
          }
        }}
      />
    </>
  );
}
