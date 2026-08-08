import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import { Loader2, RefreshCw, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  PanelEmptyState,
  PanelErrorState,
  PanelEyebrow,
  PanelListFrame,
  PanelLoadingState,
  PanelSummaryPill,
} from './panel-chrome';
import { ProcessListItem } from './process-panel/ProcessListItem';
import { ProcessOutputDialog } from './process-panel/ProcessOutputDialog';
import { useProcesses } from './process-panel/useProcesses';

interface AgentProcessPanelProps {
  isVisible?: boolean;
  /** `tab` omits outer Card chrome when hosted inside AgentSidePanelShell. */
  variant?: 'rail' | 'sheet' | 'tab';
}

/**
 * Background process panel (#1686).
 * Lists session processes via workspace__listProcesses and exposes
 * read / status-check / stop actions through the existing builtin tool bridge.
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

  const toolbar = (
    <div className="flex shrink-0 flex-col gap-2 border-b border-border/40 px-4 py-3">
      {variant !== 'tab' ? (
        <div className="flex items-center justify-between gap-2">
          <PanelEyebrow icon={<Terminal className="h-3.5 w-3.5" />}>
            {t('agent.processes.title', 'Background Processes')}
          </PanelEyebrow>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
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
      ) : (
        <div className="flex items-center justify-end">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
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
      )}

      <div
        className="flex flex-wrap gap-2"
        aria-live="polite"
        aria-atomic="true"
      >
        <PanelSummaryPill>
          {t('agent.processes.summaryTotal', '{{count}} total', {
            count: total,
          })}
        </PanelSummaryPill>
        <PanelSummaryPill>
          {t('agent.processes.summaryRunning', '{{count}} running', {
            count: running,
          })}
        </PanelSummaryPill>
        <PanelSummaryPill>
          {t('agent.processes.summaryFinished', '{{count}} finished', {
            count: finished,
          })}
        </PanelSummaryPill>
      </div>
    </div>
  );

  const body = (
    <div
      className="flex min-h-0 flex-1 flex-col gap-3 p-4"
      aria-live="polite"
      aria-relevant="additions text"
    >
      {error ? (
        <PanelErrorState
          message={error}
          retryLabel={t('agent.processes.retry', 'Retry')}
          onRetry={() => {
            void refresh();
          }}
        />
      ) : loading && processes.length === 0 ? (
        <PanelLoadingState>
          {t('agent.processes.loading', 'Loading processes…')}
        </PanelLoadingState>
      ) : processes.length === 0 ? (
        <PanelEmptyState
          icon={
            <Terminal
              className="h-5 w-5 text-muted-foreground/60"
              aria-hidden="true"
            />
          }
        >
          {t(
            'agent.processes.empty',
            'No background processes in this session yet.',
          )}
        </PanelEmptyState>
      ) : (
        <PanelListFrame>
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
        </PanelListFrame>
      )}
    </div>
  );

  const dialog = (
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
  );

  if (variant === 'tab') {
    return (
      <>
        <div
          id="agent-processes-panel"
          role="region"
          aria-label={t('agent.processes.title', 'Background Processes')}
          aria-hidden={!isVisible}
          className={cn(
            'flex h-full flex-col bg-background',
            !isVisible && 'pointer-events-none',
          )}
        >
          {toolbar}
          {body}
        </div>
        {dialog}
      </>
    );
  }

  return (
    <>
      <Card
        id="agent-processes-panel"
        role="region"
        aria-label={t('agent.processes.title', 'Background Processes')}
        aria-hidden={!isVisible}
        className={cn(
          'flex h-full flex-col rounded-none bg-background py-0 shadow-none gap-0',
          variant === 'rail'
            ? 'border-y-0 border-l border-r-0 border-border/40'
            : 'border-0',
          !isVisible && 'pointer-events-none',
        )}
      >
        <CardHeader className="shrink-0 space-y-0 p-0">{toolbar}</CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col p-0">
          {body}
        </CardContent>
      </Card>
      {dialog}
    </>
  );
}
