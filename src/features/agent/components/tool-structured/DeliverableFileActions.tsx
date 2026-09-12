import React, { useState } from 'react';
import {
  Download,
  ExternalLink,
  Eye,
  FileCode,
  FileSpreadsheet,
  FileText,
  FileArchive,
  Image as ImageIcon,
  AlertTriangle,
  Loader2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { openPathWithDefaultApp, downloadWorkspaceFile } from '@/lib/backend';
import { getLogger } from '@/lib/logger';
import { Button } from '@/components/ui/button';
import { useOptionalAgentFilePreview } from '@/context/AgentFilePreviewContext';
import { useOptionalAgentSessionState } from '@/context/AgentSessionContext';
import type { DeliverableItem } from './types';
import {
  canOpenInAppPreview,
  fileNameFromPath,
} from '../workspace-panel/filePreview';

const logger = getLogger('DeliverableFileActions');

export interface DeliverableFileActionsProps {
  item: DeliverableItem;
  sessionId?: string;
}

function getFileIcon(extension: string | null | undefined) {
  const ext = extension?.toLowerCase().trim() || '';
  if (
    ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico'].includes(ext)
  ) {
    return ImageIcon;
  }
  if (['zip', 'tar', 'gz', 'bz2', '7z', 'rar'].includes(ext)) {
    return FileArchive;
  }
  if (['csv', 'tsv', 'xlsx', 'xls'].includes(ext)) {
    return FileSpreadsheet;
  }
  if (
    [
      'ts',
      'tsx',
      'js',
      'jsx',
      'rs',
      'py',
      'json',
      'html',
      'css',
      'scss',
      'yaml',
      'yml',
      'toml',
      'sh',
      'sql',
    ].includes(ext)
  ) {
    return FileCode;
  }
  return FileText;
}

function formatBytes(bytes: number | null | undefined): string | null {
  if (bytes === undefined || bytes === null || !Number.isFinite(bytes))
    return null;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export const DeliverableFileActions: React.FC<DeliverableFileActionsProps> = ({
  item,
  sessionId: propSessionId,
}) => {
  const { t } = useTranslation('common');
  const sessionContext = useOptionalAgentSessionState();
  const filePreview = useOptionalAgentFilePreview();

  const [isOpening, setIsOpening] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);

  const activeSessionId = propSessionId || sessionContext?.session?.id;
  const Icon = getFileIcon(item.extension);
  const sizeLabel = formatBytes(item.size_bytes);
  const openPath = item.absolute_path?.trim() || '';
  const canOpen = Boolean(openPath && item.exists);
  const canPreview =
    Boolean(filePreview) &&
    item.exists &&
    canOpenInAppPreview({
      path: item.path,
      size: item.size_bytes ?? undefined,
    });

  const handlePreview = () => {
    if (!filePreview || !canPreview) return;
    filePreview.openFilePreview({
      path: item.path,
      name: item.name || fileNameFromPath(item.path),
      size: item.size_bytes ?? undefined,
      sessionId: activeSessionId,
    });
  };

  const handleOpenExternal = async () => {
    if (!canOpen || isOpening) return;
    setIsOpening(true);
    try {
      await openPathWithDefaultApp(openPath);
    } catch (error) {
      logger.error('Failed to open deliverable file', error);
      toast.error(
        t('agent.toolStructured.openFileError', 'Failed to open file'),
      );
    } finally {
      setIsOpening(false);
    }
  };

  const handleDownload = async () => {
    if (!activeSessionId) {
      toast.error(
        t(
          'agent.toolStructured.noSessionForDownload',
          'No active session found for download',
        ),
      );
      return;
    }
    if (isDownloading) return;

    setIsDownloading(true);
    try {
      const resultPath = await downloadWorkspaceFile(
        item.path,
        activeSessionId,
      );
      if (resultPath && !resultPath.includes('cancelled')) {
        toast.success(
          t('agent.toolStructured.downloadSuccess', 'Saved: {{file}}', {
            file: item.name,
          }),
        );
      }
    } catch (error) {
      logger.error('Failed to download deliverable file', error);
      toast.error(
        t('agent.toolStructured.downloadFileError', 'Failed to download file'),
      );
    } finally {
      setIsDownloading(false);
    }
  };

  return (
    <div
      data-testid="deliverable-file-item"
      className="flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 rounded-md border border-border bg-card/60 p-2.5 transition-colors hover:bg-card"
    >
      <div className="flex items-start gap-2.5 min-w-0 flex-1">
        <div className="mt-0.5 rounded p-1 bg-muted/80 text-foreground shrink-0">
          <Icon className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono text-xs font-semibold text-foreground truncate max-w-[280px]">
              {item.name}
            </span>
            {sizeLabel ? (
              <span className="text-[11px] text-muted-foreground">
                {sizeLabel}
              </span>
            ) : null}
            {!item.exists ? (
              <span className="inline-flex items-center gap-1 rounded bg-destructive/10 px-1.5 py-0.5 text-[11px] font-medium text-destructive">
                <AlertTriangle className="h-3 w-3" />
                {t('agent.toolStructured.fileNotFound', 'Not found')}
              </span>
            ) : null}
          </div>
          <p className="font-mono text-[11px] text-muted-foreground break-all mt-0.5">
            {item.path}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-1.5 shrink-0 self-end sm:self-center">
        {canPreview ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 text-xs px-2"
            onClick={handlePreview}
            data-testid="deliverable-preview-button"
          >
            <Eye className="mr-1 h-3.5 w-3.5" />
            {t('agent.workspace.previewMode', 'Preview')}
          </Button>
        ) : null}

        {canOpen ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 text-xs px-2"
            onClick={handleOpenExternal}
            disabled={isOpening}
            data-testid="deliverable-open-button"
          >
            {isOpening ? (
              <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
            ) : (
              <ExternalLink className="mr-1 h-3.5 w-3.5" />
            )}
            {t('agent.toolStructured.openWithApp', 'Open')}
          </Button>
        ) : null}

        {item.exists ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 text-xs px-2"
            onClick={handleDownload}
            disabled={isDownloading}
            data-testid="deliverable-download-button"
          >
            {isDownloading ? (
              <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
            ) : (
              <Download className="mr-1 h-3.5 w-3.5" />
            )}
            {t('common.download', 'Download')}
          </Button>
        ) : null}
      </div>
    </div>
  );
};
