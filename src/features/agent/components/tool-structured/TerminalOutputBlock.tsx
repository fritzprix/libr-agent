import React from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { formatExecutionTime } from '@/lib/tool-call-utils';
import type { RunShellResult } from './types';

export interface TerminalOutputBlockProps {
  data: RunShellResult;
}

/**
 * Structured result view for workspace__runShell success payloads.
 */
export const TerminalOutputBlock: React.FC<TerminalOutputBlockProps> = ({
  data,
}) => {
  const { t } = useTranslation('common');
  const exitOk = data.exit_code === 0;
  const hasStdout = data.stdout.trim().length > 0;
  const hasStderr = data.stderr.trim().length > 0;

  return (
    <div
      data-testid="tool-structured-run-shell"
      className="space-y-2 overflow-hidden rounded border bg-zinc-950 text-zinc-100"
    >
      <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800 px-3 py-2 text-xs">
        <span
          className={cn(
            'rounded px-1.5 py-0.5 font-medium',
            exitOk
              ? 'bg-emerald-500/20 text-emerald-300'
              : 'bg-red-500/20 text-red-300',
          )}
        >
          {t('agent.toolStructured.exitCode', 'exit {{code}}', {
            code: data.exit_code,
          })}
        </span>
        {data.duration_ms !== undefined ? (
          <span className="text-zinc-400">
            {formatExecutionTime(data.duration_ms)}
          </span>
        ) : null}
        {data.cwd ? (
          <span className="truncate text-zinc-500" title={data.cwd}>
            {data.cwd}
          </span>
        ) : null}
      </div>

      <div className="px-3 pb-1">
        <pre className="overflow-x-auto whitespace-pre-wrap break-all font-mono text-xs text-zinc-200">
          <span className="select-none text-emerald-400">$ </span>
          {data.command}
        </pre>
      </div>

      {hasStdout ? (
        <div className="border-t border-zinc-800 px-3 py-2">
          <div className="mb-1 text-[10px] uppercase tracking-wide text-zinc-500">
            {t('agent.toolStructured.stdout', 'stdout')}
          </div>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-zinc-100">
            {data.stdout}
          </pre>
        </div>
      ) : null}

      {hasStderr ? (
        <div className="border-t border-zinc-800 px-3 py-2">
          <div className="mb-1 text-[10px] uppercase tracking-wide text-red-400/80">
            {t('agent.toolStructured.stderr', 'stderr')}
          </div>
          <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-red-200">
            {data.stderr}
          </pre>
        </div>
      ) : null}

      {!hasStdout && !hasStderr ? (
        <div className="border-t border-zinc-800 px-3 py-2 text-xs text-zinc-500">
          {t('agent.toolStructured.noShellOutput', 'No output captured')}
        </div>
      ) : null}
    </div>
  );
};
