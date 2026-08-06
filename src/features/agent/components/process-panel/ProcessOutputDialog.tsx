import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { ScrollArea } from '@/components/ui';
import { AlertTriangle, Loader2 } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { ReadProcessOutputResult } from './types';

interface ProcessOutputDialogProps {
  open: boolean;
  loading: boolean;
  result: ReadProcessOutputResult | null;
  error?: string | null;
  onOpenChange: (open: boolean) => void;
}

function renderStream(
  label: string,
  lines: string[] | undefined,
  emptyLabel: string,
) {
  const text = lines && lines.length > 0 ? lines.join('\n') : emptyLabel;

  return (
    <section className="space-y-2">
      <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </h4>
      <pre className="rounded-md border border-border/40 bg-muted/20 p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap break-all text-foreground/90">
        {text}
      </pre>
    </section>
  );
}

export function ProcessOutputDialog({
  open,
  loading,
  result,
  error = null,
  onOpenChange,
}: ProcessOutputDialogProps) {
  const { t } = useTranslation();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open || loading || (!result && !error)) {
      return;
    }
    const node = bottomRef.current;
    if (node && typeof node.scrollIntoView === 'function') {
      node.scrollIntoView({ block: 'end' });
    }
  }, [open, loading, result, error]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] w-[calc(100%-2rem)] max-w-2xl flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>
            {t('agent.processes.outputTitle', 'Process output')}
            {result?.process_id ? (
              <span className="ml-2 font-mono text-xs font-normal text-muted-foreground">
                {result.process_id}
              </span>
            ) : null}
          </DialogTitle>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t('agent.processes.loadingOutput', 'Loading output…')}
          </div>
        ) : (
          <ScrollArea className="min-h-0 flex-1 pr-3">
            <div className="space-y-4 pb-2">
              {error ? (
                <div
                  role="alert"
                  className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive"
                >
                  <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>{error}</span>
                </div>
              ) : null}
              {result ? (
                <>
                  <p className="text-xs text-muted-foreground">
                    {t('agent.processes.outputMeta', {
                      defaultValue: 'Status: {{status}} · stream: {{stream}}',
                      status: result.status,
                      stream: result.stream,
                    })}
                  </p>
                  {renderStream(
                    'stdout',
                    result.outputs.stdout?.content,
                    t('agent.processes.emptyStream', '(empty)'),
                  )}
                  {renderStream(
                    'stderr',
                    result.outputs.stderr?.content,
                    t('agent.processes.emptyStream', '(empty)'),
                  )}
                </>
              ) : !error ? (
                <p className="py-8 text-center text-sm text-muted-foreground">
                  {t('agent.processes.noOutput', 'No output available')}
                </p>
              ) : null}
              <div ref={bottomRef} aria-hidden />
            </div>
          </ScrollArea>
        )}
      </DialogContent>
    </Dialog>
  );
}
