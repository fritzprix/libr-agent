import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, FileDown, FileJson, FileText, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { useClipboard } from '@/hooks/useClipboard';
import { exportSessionFile } from '@/lib/backend';
import { getLogger } from '@/lib/logger';
import { messagesToMarkdown } from '@/lib/message-utils';
import {
  DOWNLOAD_CANCELLED,
  notifyFileDownloadSuccess,
} from '@/lib/notify-file-download';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';

const logger = getLogger('SessionExportMenu');

type BusyAction = 'markdown' | 'atif' | 'clipboard' | null;

export interface SessionExportMenuProps {
  sessionId: string;
  messages?: Message[];
  showClipboardCopy?: boolean;
  className?: string;
}

export function SessionExportMenu({
  sessionId,
  messages = [],
  showClipboardCopy = false,
  className,
}: SessionExportMenuProps) {
  const { t } = useTranslation();
  const { copyToClipboard } = useClipboard();
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const isBusy = busyAction !== null;

  const handleFileExport = useCallback(
    async (format: 'markdown' | 'atif') => {
      if (isBusy) {
        return;
      }
      setBusyAction(format);
      try {
        const result = await exportSessionFile({ sessionId, format });
        if (result === DOWNLOAD_CANCELLED) {
          toast.info(t('agent.sessionExport.cancelled'));
          return;
        }
        notifyFileDownloadSuccess({
          title:
            format === 'markdown'
              ? t('agent.sessionExport.markdownSuccess')
              : t('agent.sessionExport.atifSuccess'),
          filePath: result,
          openLabel: t('agent.sessionExport.openFile'),
          openErrorLabel: t('agent.sessionExport.openFileError'),
        });
      } catch (error) {
        logger.error('Failed to export session', { sessionId, format, error });
        toast.error(t('agent.sessionExport.error'));
      } finally {
        setBusyAction(null);
      }
    },
    [isBusy, sessionId, t],
  );

  const handleCopyClipboard = useCallback(async () => {
    if (isBusy) {
      return;
    }
    setBusyAction('clipboard');
    try {
      const { content, truncated } = messagesToMarkdown(messages);
      await copyToClipboard(content);
      toast.success(
        truncated
          ? t('agent.header.copySuccessPartial')
          : t('agent.header.copySuccess'),
      );
    } catch (error) {
      logger.error('Failed to copy session messages', { sessionId, error });
      if (error instanceof DOMException && error.name === 'NotAllowedError') {
        toast.error(t('agent.header.copyDenied'));
      } else {
        toast.error(t('agent.header.copyError'));
      }
    } finally {
      setBusyAction(null);
    }
  }, [copyToClipboard, isBusy, messages, sessionId, t]);

  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={isBusy}
              aria-label={t('agent.sessionExport.menuAria')}
              className={cn(
                'h-6 px-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                className,
              )}
            >
              {isBusy ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <FileDown className="h-4 w-4" />
              )}
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{t('agent.sessionExport.menuTooltip')}</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" className="min-w-[14rem]">
        <DropdownMenuItem
          disabled={isBusy}
          onSelect={() => {
            void handleFileExport('markdown');
          }}
        >
          <FileText className="h-4 w-4" />
          {t('agent.sessionExport.markdown')}
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={isBusy}
          onSelect={() => {
            void handleFileExport('atif');
          }}
        >
          <FileJson className="h-4 w-4" />
          {t('agent.sessionExport.atif')}
        </DropdownMenuItem>
        {showClipboardCopy ? (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              disabled={isBusy}
              onSelect={() => {
                void handleCopyClipboard();
              }}
            >
              <Copy className="h-4 w-4" />
              {t('agent.sessionExport.copyClipboard')}
            </DropdownMenuItem>
          </>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
