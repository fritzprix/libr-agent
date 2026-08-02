import { memo, useCallback, useMemo, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Braces,
  Check,
  ChevronDown,
  Copy,
  Download,
  FileDown,
  Loader2,
  Printer,
  Type,
} from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { useClipboard } from '@/hooks/useClipboard';
import { downloadTextFile, downloadTextPdf } from '@/lib/backend';
import { getLogger } from '@/lib/logger';
import {
  DOWNLOAD_CANCELLED,
  notifyFileDownloadSuccess,
} from '@/lib/notify-file-download';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import {
  buildMessageExportFilename,
  serializeMessageForClipboard,
  serializeMessageForDownload,
} from '@/features/agent/lib/message-serialization';

const logger = getLogger('MessageActionBar');

type BusyAction = 'full' | 'text' | 'tools' | 'markdown' | 'pdf' | null;
type CopyMode = 'full' | 'text' | 'tools';

export interface MessageActionBarProps {
  message: Message;
  displayContent?: MCPContent[];
  toolResultsMap?: Map<string, Message>;
  /** Visual tone for user (primary) vs assistant/secondary bubbles */
  tone?: 'user' | 'assistant';
  className?: string;
}

function IconActionButton({
  label,
  tooltip,
  onClick,
  disabled,
  isBusy,
  showCheck,
  emphasize,
  isUserTone,
  children,
}: {
  label: string;
  tooltip: string;
  onClick: () => void;
  disabled: boolean;
  isBusy: boolean;
  showCheck: boolean;
  emphasize?: boolean;
  isUserTone: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className={cn(
            'h-7 w-7 px-0',
            isUserTone
              ? emphasize
                ? 'text-primary-foreground hover:bg-primary-foreground/15 hover:text-primary-foreground'
                : 'text-primary-foreground/75 hover:bg-primary-foreground/15 hover:text-primary-foreground'
              : emphasize
                ? 'text-foreground hover:text-foreground'
                : 'text-muted-foreground hover:text-foreground',
          )}
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
        >
          {isBusy ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : showCheck ? (
            <Check className="h-3.5 w-3.5" />
          ) : (
            children
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{tooltip}</TooltipContent>
    </Tooltip>
  );
}

function MessageActionBarImpl({
  message,
  displayContent,
  toolResultsMap,
  tone = 'assistant',
  className,
}: MessageActionBarProps) {
  const { t } = useTranslation();
  const { copyToClipboard } = useClipboard();
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [lastCopiedMode, setLastCopiedMode] = useState<CopyMode | null>(null);

  const isBusy = busyAction !== null;
  const isUserTone = tone === 'user';

  const hasToolCalls = useMemo(() => {
    if ((message.tool_calls?.length ?? 0) > 0) {
      return true;
    }
    const content = displayContent ?? message.content ?? [];
    return content.some((item) => item.type === 'tool_call');
  }, [displayContent, message.content, message.tool_calls]);

  const serialize = useCallback(
    (mode: CopyMode) =>
      serializeMessageForClipboard(message, {
        mode,
        displayContent,
        toolResultsMap,
        includeThinking: true,
        includeToolCalls: true,
        includeToolResults: true,
      }),
    [displayContent, message, toolResultsMap],
  );

  const handleCopy = useCallback(
    async (mode: CopyMode) => {
      if (isBusy) {
        return;
      }
      setBusyAction(mode);
      try {
        const content = serialize(mode);
        if (!content.trim() || content === '[]') {
          toast.error(t('agent.bubble.actionBar.copyEmpty'));
          return;
        }
        await copyToClipboard(content);
        setLastCopiedMode(mode);
        toast.success(t('agent.bubble.actionBar.copySuccess'));
      } catch (error) {
        logger.error('Failed to copy message', error);
        if (error instanceof DOMException && error.name === 'NotAllowedError') {
          toast.error(t('agent.bubble.actionBar.copyDenied'));
        } else {
          toast.error(t('agent.bubble.actionBar.copyError'));
        }
      } finally {
        setBusyAction(null);
      }
    },
    [copyToClipboard, isBusy, serialize, t],
  );

  const exportMarkdownContent = useCallback(
    () =>
      serializeMessageForDownload(message, {
        displayContent,
      }),
    [displayContent, message],
  );

  const handleExportMarkdown = useCallback(async () => {
    if (isBusy) {
      return;
    }
    setBusyAction('markdown');
    try {
      const content = exportMarkdownContent();
      if (!content.trim()) {
        toast.error(t('agent.bubble.actionBar.copyEmpty'));
        return;
      }
      const result = await downloadTextFile({
        fileName: buildMessageExportFilename(message, 'md'),
        content,
      });
      if (result === DOWNLOAD_CANCELLED) {
        toast.info(t('agent.bubble.actionBar.exportCancelled'));
        return;
      }
      notifyFileDownloadSuccess({
        title: t('agent.bubble.actionBar.exportMarkdownSuccess'),
        filePath: result,
        openLabel: t('agent.bubble.actionBar.exportOpenFile'),
        openErrorLabel: t('agent.bubble.actionBar.exportOpenFileError'),
      });
    } catch (error) {
      logger.error('Failed to export markdown', error);
      toast.error(t('agent.bubble.actionBar.exportError'));
    } finally {
      setBusyAction(null);
    }
  }, [exportMarkdownContent, isBusy, message, t]);

  const handleExportPdf = useCallback(async () => {
    if (isBusy) {
      return;
    }
    setBusyAction('pdf');
    try {
      const content = exportMarkdownContent();
      if (!content.trim()) {
        toast.error(t('agent.bubble.actionBar.copyEmpty'));
        return;
      }
      const result = await downloadTextPdf({
        fileName: buildMessageExportFilename(message, 'pdf'),
        content,
      });
      if (result === DOWNLOAD_CANCELLED) {
        toast.info(t('agent.bubble.actionBar.exportCancelled'));
        return;
      }
      notifyFileDownloadSuccess({
        title: t('agent.bubble.actionBar.exportPdfSuccess'),
        filePath: result,
        openLabel: t('agent.bubble.actionBar.exportOpenFile'),
        openErrorLabel: t('agent.bubble.actionBar.exportOpenFileError'),
      });
    } catch (error) {
      logger.error('Failed to export PDF', error);
      toast.error(t('agent.bubble.actionBar.exportPdfError'));
    } finally {
      setBusyAction(null);
    }
  }, [exportMarkdownContent, isBusy, message, t]);

  return (
    <div
      className={cn('mt-1.5 flex items-center gap-0.5', className)}
      data-testid="message-action-bar"
    >
      <IconActionButton
        label={t('agent.bubble.actionBar.copyFullAria')}
        tooltip={t('agent.bubble.actionBar.copyFullTooltip')}
        onClick={() => {
          void handleCopy('full');
        }}
        disabled={isBusy}
        isBusy={busyAction === 'full'}
        showCheck={lastCopiedMode === 'full' && busyAction !== 'full'}
        emphasize
        isUserTone={isUserTone}
      >
        <Copy className="h-3.5 w-3.5" />
      </IconActionButton>

      <IconActionButton
        label={t('agent.bubble.actionBar.copyTextAria')}
        tooltip={t('agent.bubble.actionBar.copyTextTooltip')}
        onClick={() => {
          void handleCopy('text');
        }}
        disabled={isBusy}
        isBusy={busyAction === 'text'}
        showCheck={lastCopiedMode === 'text' && busyAction !== 'text'}
        isUserTone={isUserTone}
      >
        <Type className="h-3.5 w-3.5" />
      </IconActionButton>

      <IconActionButton
        label={t('agent.bubble.actionBar.copyToolsAria')}
        tooltip={t('agent.bubble.actionBar.copyToolsTooltip')}
        onClick={() => {
          void handleCopy('tools');
        }}
        disabled={isBusy || !hasToolCalls}
        isBusy={busyAction === 'tools'}
        showCheck={lastCopiedMode === 'tools' && busyAction !== 'tools'}
        isUserTone={isUserTone}
      >
        <Braces className="h-3.5 w-3.5" />
      </IconActionButton>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className={cn(
                  'h-7 gap-0.5 px-1.5',
                  isUserTone
                    ? 'text-primary-foreground/75 hover:bg-primary-foreground/15 hover:text-primary-foreground'
                    : 'text-muted-foreground hover:text-foreground',
                )}
                disabled={isBusy}
                aria-label={t('agent.bubble.actionBar.exportAria')}
              >
                {busyAction === 'markdown' || busyAction === 'pdf' ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Download className="h-3.5 w-3.5" />
                )}
                <ChevronDown className="h-3 w-3 opacity-70" />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>
            {t('agent.bubble.actionBar.exportTooltip')}
          </TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="start" className="min-w-[11rem]">
          <DropdownMenuItem
            onSelect={() => {
              void handleExportMarkdown();
            }}
          >
            <FileDown className="h-4 w-4" />
            {t('agent.bubble.actionBar.exportMarkdown')}
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => {
              void handleExportPdf();
            }}
          >
            <Printer className="h-4 w-4" />
            {t('agent.bubble.actionBar.exportPdf')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

export const MessageActionBar = memo(MessageActionBarImpl);
