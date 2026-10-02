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

type BusyAction = 'copy' | 'tools' | 'markdown' | 'pdf' | null;

export interface MessageActionBarProps {
  message: Message;
  displayContent?: MCPContent[];
  toolResultsMap?: Map<string, Message>;
  /** Visual tone for user (primary) vs assistant/secondary bubbles */
  tone?: 'user' | 'assistant';
  className?: string;
}

/**
 * One copy payload for the bubble: UI document if present, else plain text.
 * Same rule for reportResult / presentInteractive and normal messages.
 */
function resolveBubbleCopyBody(
  uiDocument: ReturnType<typeof resolveMessageDocument>,
  message: Message,
  displayContent?: MCPContent[],
): string {
  const fromDocument = uiDocument?.content.trim() ?? '';
  if (fromDocument) {
    return fromDocument;
  }
  return serializeMessageTextOnly(message, displayContent).trim();
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
  const { copied, copyToClipboard } = useClipboard();
  const isDark = useIsDarkMode();
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [lastCopied, setLastCopied] = useState<'copy' | 'tools' | null>(null);

  const isBusy = busyAction !== null;
  const isUserTone = tone === 'user';

  const hasToolCalls = useMemo(() => {
    if ((message.tool_calls?.length ?? 0) > 0) {
      return true;
    }
    const content = displayContent ?? message.content ?? [];
    return content.some((item) => item.type === 'tool_call');
  }, [displayContent, message.content, message.tool_calls]);

  const uiDocument = useMemo(
    () =>
      resolveMessageDocument(message, {
        displayContent,
        toolResultsMap,
      }),
    [displayContent, message, toolResultsMap],
  );

  const bubbleCopyBody = useMemo(
    () => resolveBubbleCopyBody(uiDocument, message, displayContent),
    [displayContent, message, uiDocument],
  );
  const canCopyBody = bubbleCopyBody.length > 0;
  const canExportBody = canCopyBody;

  const handleCopyBody = useCallback(async () => {
    if (isBusy || !canCopyBody) {
      return;
    }
    setBusyAction('copy');
    try {
      await copyToClipboard(bubbleCopyBody);
      setLastCopied('copy');
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
  }, [bubbleCopyBody, canCopyBody, copyToClipboard, isBusy, t]);

  const handleCopyTools = useCallback(async () => {
    if (isBusy || !hasToolCalls) {
      return;
    }
    setBusyAction('tools');
    try {
      const content = serializeMessageForClipboard(message, {
        mode: 'tools',
        displayContent,
        toolResultsMap,
        includeThinking: true,
        includeToolCalls: true,
        includeToolResults: true,
      });
      if (!content.trim() || content === '[]') {
        toast.error(t('agent.bubble.actionBar.copyEmpty'));
        return;
      }
      await copyToClipboard(content);
      setLastCopied('tools');
      toast.success(t('agent.bubble.actionBar.copySuccess'));
    } catch (error) {
      logger.error('Failed to copy tools', error);
      if (error instanceof DOMException && error.name === 'NotAllowedError') {
        toast.error(t('agent.bubble.actionBar.copyDenied'));
      } else {
        toast.error(t('agent.bubble.actionBar.copyError'));
      }
    } finally {
      setBusyAction(null);
    }
  }, [
    copyToClipboard,
    displayContent,
    hasToolCalls,
    isBusy,
    message,
    t,
    toolResultsMap,
  ]);

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
        label={t('agent.bubble.actionBar.copyAria')}
        tooltip={t('agent.bubble.actionBar.copyTooltip')}
        onClick={() => {
          void handleCopyBody();
        }}
        disabled={isBusy || !canCopyBody}
        isBusy={busyAction === 'copy'}
        showCheck={copied && lastCopied === 'copy' && busyAction !== 'copy'}
        emphasize
        isUserTone={isUserTone}
      >
        <Copy className="h-3.5 w-3.5" />
      </IconActionButton>

      <IconActionButton
        label={t('agent.bubble.actionBar.copyToolsAria')}
        tooltip={t('agent.bubble.actionBar.copyToolsTooltip')}
        onClick={() => {
          void handleCopyTools();
        }}
        disabled={isBusy || !hasToolCalls}
        isBusy={busyAction === 'tools'}
        showCheck={copied && lastCopied === 'tools' && busyAction !== 'tools'}
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
