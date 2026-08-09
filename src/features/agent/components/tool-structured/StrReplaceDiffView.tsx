import React from 'react';
import { FilePenLine } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { UnifiedDiffView } from './UnifiedDiffView';
import type { StrReplaceResult } from './types';

export interface StrReplaceDiffViewProps {
  data: StrReplaceResult;
}

/**
 * Structured result view for workspace__strReplace.
 */
export const StrReplaceDiffView: React.FC<StrReplaceDiffViewProps> = ({
  data,
}) => {
  const { t } = useTranslation('common');

  return (
    <div
      data-testid="tool-structured-str-replace"
      className="space-y-2 text-sm"
    >
      <div className="flex items-start gap-2">
        <FilePenLine className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded bg-muted px-1.5 py-0.5 text-xs font-medium">
              {t('agent.toolStructured.strReplace', 'Replaced')}
            </span>
            <span className="text-xs text-muted-foreground">
              {t(
                'agent.toolStructured.replacementCount',
                '{{count}} occurrence(s)',
                { count: data.replacements },
              )}
            </span>
          </div>
          <p className="break-all font-mono text-xs">{data.path}</p>
        </div>
      </div>
      <UnifiedDiffView diff={data.unified_diff} />
    </div>
  );
};
