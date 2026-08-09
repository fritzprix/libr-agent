import React, { useState } from 'react';
import { ExternalLink, FilePlus2, FilePenLine, FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { openPathWithDefaultApp } from '@/lib/backend';
import { getLogger } from '@/lib/logger';
import { Button } from '@/components/ui/button';
import { UnifiedDiffView } from './UnifiedDiffView';
import type { WriteFileResult } from './types';

const logger = getLogger('FileWriteActions');

export interface FileWriteActionsProps {
  data: WriteFileResult;
}

function actionIcon(action: WriteFileResult['action']) {
  switch (action) {
    case 'created':
    case 'created_alternate_path':
      return FilePlus2;
    case 'overwritten':
      return FilePenLine;
    default:
      return FileText;
  }
}

function formatBytes(bytes: number | undefined): string | null {
  if (bytes === undefined || !Number.isFinite(bytes)) return null;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Structured result view for workspace__writeFile.
 */
export const FileWriteActions: React.FC<FileWriteActionsProps> = ({ data }) => {
  const { t } = useTranslation('common');
  const [isOpening, setIsOpening] = useState(false);
  const Icon = actionIcon(data.action);
  const sizeLabel = formatBytes(data.bytes_written);
  // open_path_with_default_app requires an absolute host path
  const openPath = data.absolute_path?.trim() || '';
  const canOpen = openPath.length > 0;

  const actionLabel = (() => {
    switch (data.action) {
      case 'created':
        return t('agent.toolStructured.writeCreated', 'Created');
      case 'created_alternate_path':
        return t(
          'agent.toolStructured.writeCreatedAlternate',
          'Created at alternate path',
        );
      case 'overwritten':
        return t('agent.toolStructured.writeOverwritten', 'Overwritten');
      case 'appended':
        return t('agent.toolStructured.writeAppended', 'Appended');
      default:
        return data.action;
    }
  })();

  const handleOpen = async () => {
    if (!canOpen || isOpening) return;
    setIsOpening(true);
    try {
      await openPathWithDefaultApp(openPath);
    } catch (error) {
      logger.error('Failed to open written file', error);
      toast.error(
        t('agent.toolStructured.openFileError', 'Failed to open file'),
      );
    } finally {
      setIsOpening(false);
    }
  };

  return (
    <div data-testid="tool-structured-write-file" className="space-y-2 text-sm">
      <div className="flex items-start gap-2">
        <Icon className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded bg-muted px-1.5 py-0.5 text-xs font-medium">
              {actionLabel}
            </span>
            {sizeLabel ? (
              <span className="text-xs text-muted-foreground">{sizeLabel}</span>
            ) : null}
            {data.lines !== undefined ? (
              <span className="text-xs text-muted-foreground">
                {t('agent.toolStructured.lineCount', '{{count}} lines', {
                  count: data.lines,
                })}
              </span>
            ) : null}
            {data.changes?.lines_added !== undefined ||
            data.changes?.lines_removed !== undefined ? (
              <span className="text-xs text-muted-foreground">
                +{data.changes.lines_added ?? 0}/-
                {data.changes.lines_removed ?? 0}
              </span>
            ) : null}
          </div>
          <p className="break-all font-mono text-xs">{data.path}</p>
          {data.path_adjusted && data.requested_path ? (
            <p className="text-xs text-muted-foreground">
              {t('agent.toolStructured.requestedPath', 'Requested: {{path}}', {
                path: data.requested_path,
              })}
            </p>
          ) : null}
        </div>
        {canOpen ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={() => {
              void handleOpen();
            }}
            disabled={isOpening}
          >
            <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
            {isOpening
              ? t('agent.toolStructured.openingFile', 'Opening...')
              : t('agent.toolStructured.openFile', 'Open file')}
          </Button>
        ) : null}
      </div>

      {data.unified_diff ? <UnifiedDiffView diff={data.unified_diff} /> : null}
    </div>
  );
};
