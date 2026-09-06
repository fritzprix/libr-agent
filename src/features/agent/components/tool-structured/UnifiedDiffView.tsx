import React, { useMemo } from 'react';
import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';

export interface UnifiedDiffViewProps {
  diff: string;
  className?: string;
}

type DiffLineKind = 'add' | 'remove' | 'hunk' | 'meta' | 'context';

function classifyDiffLine(line: string): DiffLineKind {
  if (line.startsWith('+++') || line.startsWith('---')) return 'meta';
  if (line.startsWith('@@')) return 'hunk';
  if (line.startsWith('+')) return 'add';
  if (line.startsWith('-')) return 'remove';
  return 'context';
}

const LINE_CLASS: Record<DiffLineKind, string> = {
  add: 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
  remove: 'bg-destructive/10 text-destructive',
  hunk: 'bg-muted/60 text-muted-foreground',
  meta: 'text-muted-foreground',
  context: 'text-foreground/90',
};

/**
 * Renders a unified diff string with add/remove/context coloring.
 */
export const UnifiedDiffView: React.FC<UnifiedDiffViewProps> = ({
  diff,
  className,
}) => {
  const { t } = useTranslation('common');
  const lines = useMemo(() => diff.replace(/\r\n/g, '\n').split('\n'), [diff]);
  const wasTruncated = lines.some((line) =>
    /more changed line\(s\) omitted/i.test(line),
  );

  if (!diff.trim()) {
    return (
      <p className="text-xs text-muted-foreground">
        {t('agent.toolStructured.diffEmpty', 'No diff to display')}
      </p>
    );
  }

  return (
    <div className={cn('space-y-1', className)}>
      {wasTruncated ? (
        <p className="text-xs text-muted-foreground">
          {t(
            'agent.toolStructured.diffTruncated',
            'Diff preview was truncated by the tool',
          )}
        </p>
      ) : null}
      <div className="max-h-80 overflow-auto rounded border bg-muted/30">
        <pre className="min-w-max p-2 font-mono text-xs leading-5">
          {lines.map((line, index) => {
            const kind = classifyDiffLine(line);
            return (
              <div
                key={`${index}-${line.slice(0, 24)}`}
                className={cn('whitespace-pre px-1', LINE_CLASS[kind])}
              >
                {line.length > 0 ? line : ' '}
              </div>
            );
          })}
        </pre>
      </div>
    </div>
  );
};
