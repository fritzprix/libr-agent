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
import { useIsDarkMode } from '@/hooks/use-is-dark-mode';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import {
  buildMessageExportFilename,
  serializeMessageForClipboard,
  serializeMessageForDownload,
  serializeMessageTextOnly,
} from '@/features/agent/lib/message-serialization';
import {
  exportMarkdownDocumentWithNotify,
  markdownExportBaseName,
  markdownExportLabelsFromT,
} from '@/features/agent/lib/markdown-document-export';
import { resolveMessageDocument } from '@/features/agent/lib/message-document';

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

/**
 * Bubble-local payloads: UI document wins over plain transcript text.
 * Never walks other bubbles' history.
 */
function resolveBubbleCopyPayloads(
  uiDocument: ReturnType<typeof resolveMessageDocument>,
  message: Message,
  displayContent?: MCPContent[],
): { fullBody: string; textBody: string } {
  if (uiDocument) {
    return {
      fullBody: uiDocument.content.trim(),
      textBody: uiDocument.textBody.trim(),
    };
  }
  const plain = serializeMessageTextOnly(message, displayContent).trim();
  return { fullBody: plain, textBody: plain };
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
  const isDark = useIsDarkMode();
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

  const uiDocument = useMemo(
    () =>
      resolveMessageDocument(message, {
        displayContent,
        toolResultsMap,
      }),
    [displayContent, message, toolResultsMap],
  );

  // Priority within this bubble only: UI document → plain text.
  const { fullBody: bubbleFullBody, textBody: bubbleTextBody } = useMemo(
    () => resolveBubbleCopyPayloads(uiDocument, message, displayContent),
    [displayContent, message, uiDocument],
  );
  const canCopyBody = bubbleTextBody.length > 0;
  const canCopyFull =
    bubbleFullBody.length > 0 ||
    hasToolCalls ||
    Boolean(message.thinking?.trim());
  const canExportBody = canCopyBody || bubbleFullBody.length > 0;

  const handleCopy = useCallback(
    async (mode: CopyMode) => {
      if (isBusy) {
        return;
      }
      if (mode === 'tools' && !hasToolCalls) {
        return;
      }
      if (mode === 'text' && !canCopyBody) {
        return;
      }
      if (mode === 'full' && !canCopyFull) {
        return;
      }

      setBusyAction(mode);
      try {
        // text → document textBody or plain text
        // full → document content when present, else full transcript
        const content =
          mode === 'tools'
            ? serialize('tools')
            : mode === 'text'
              ? bubbleTextBody
              : uiDocument
                ? bubbleFullBody
                : serialize('full');
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
    [
      bubbleFullBody,
      bubbleTextBody,
      canCopyBody,
      canCopyFull,
      copyToClipboard,
      hasToolCalls,
      isBusy,
      serialize,
      t,
      uiDocument,
    ],
  );

  const exportMarkdownContent = useCallback(
    () =>
      serializeMessageForDownload(message, {
        displayContent,
        toolResultsMap,
      }),
    [displayContent, message, toolResultsMap],
  );

  const exportFileBaseName = useCallback(() => {
    if (uiDocument?.fileBaseName) {
      return markdownExportBaseName(uiDocument.fileBaseName, 'export');
    }
    // Keep message.<ext> naming; shared helper appends the extension.
    return markdownExportBaseName(
      buildMessageExportFilename(message, 'md').replace(/\.md$/i, ''),
      'message',
    );
  }, [message, uiDocument]);

  const handleExportMarkdown = useCallback(async () => {
    if (isBusy || !canExportBody) {
      return;
    }
    setBusyAction('markdown');
    try {
      const content = exportMarkdownContent();
      if (!content.trim()) {
        toast.error(t('agent.bubble.actionBar.copyEmpty'));
        return;
      }
      await exportMarkdownDocumentWithNotify({
        content,
        kind: uiDocument?.exportKind === 'html' ? 'html' : 'markdown',
        fileBaseName: exportFileBaseName(),
        isDark,
        labels: markdownExportLabelsFromT(t),
      });
    } catch {
      // Toast already shown in shared helper.
    } finally {
      setBusyAction(null);
    }
  }, [
    canExportBody,
    exportFileBaseName,
    exportMarkdownContent,
    isBusy,
    isDark,
    t,
    uiDocument?.exportKind,
  ]);

  const handleExportPdf = useCallback(async () => {
    if (isBusy || !canExportBody || uiDocument?.exportKind === 'html') {
      // PDF is hidden for HTML presentInteractive; no-op if reached.
      return;
    }
    setBusyAction('pdf');
    try {
      const content = exportMarkdownContent();
      if (!content.trim()) {
        toast.error(t('agent.bubble.actionBar.copyEmpty'));
        return;
      }
      await exportMarkdownDocumentWithNotify({
        content,
        kind: 'pdf',
        fileBaseName: exportFileBaseName(),
        isDark,
        labels: markdownExportLabelsFromT(t),
      });
    } catch {
      // Toast already shown in shared helper.
    } finally {
      setBusyAction(null);
    }
  }, [
    canExportBody,
    exportFileBaseName,
    exportMarkdownContent,
    isBusy,
    isDark,
    t,
    uiDocument?.exportKind,
  ]);

  return (
    <div
      className={cn('mt-1.5 flex items-center gap-0.5', className)}
      data-testid="message-action-bar"
    >
      <IconActionButton
        label={t('agent.bubble.actionBar.copyFullAria')}
        tooltip={
          uiDocument
            ? t('agent.bubble.actionBar.copyFullDocumentTooltip')
            : t('agent.bubble.actionBar.copyFullTooltip')
        }
        onClick={() => {
          void handleCopy('full');
        }}
        disabled={isBusy || !canCopyFull}
        isBusy={busyAction === 'full'}
        showCheck={lastCopiedMode === 'full' && busyAction !== 'full'}
        emphasize
        isUserTone={isUserTone}
      >
        <Copy className="h-3.5 w-3.5" />
      </IconActionButton>

      <IconActionButton
        label={t('agent.bubble.actionBar.copyTextAria')}
        tooltip={
          uiDocument
            ? t('agent.bubble.actionBar.copyTextDocumentTooltip')
            : t('agent.bubble.actionBar.copyTextTooltip')
        }
        onClick={() => {
          void handleCopy('text');
        }}
        disabled={isBusy || !canCopyBody}
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
                disabled={isBusy || !canExportBody}
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
            disabled={!canExportBody}
          >
            <FileDown className="h-4 w-4" />
            {uiDocument?.exportKind === 'html'
              ? t('agent.bubble.actionBar.exportHtml')
              : t('agent.bubble.actionBar.exportMarkdown')}
          </DropdownMenuItem>
          {uiDocument?.exportKind === 'html' ? null : (
            <DropdownMenuItem
              onSelect={() => {
                void handleExportPdf();
              }}
              disabled={!canExportBody}
            >
              <Printer className="h-4 w-4" />
              {t('agent.bubble.actionBar.exportPdf')}
            </DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

export const MessageActionBar = memo(MessageActionBarImpl);
