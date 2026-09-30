import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
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
import { useIsDarkMode } from '@/hooks/use-is-dark-mode';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import {
  exportMarkdownDocumentWithNotify,
  markdownExportBaseName,
  markdownExportLabelsFromT,
  type MarkdownExportKind,
} from '@/features/agent/lib/markdown-document-export';

const logger = getLogger('MarkdownCopyExportBar');
const COPY_FEEDBACK_MS = 2000;

export interface MarkdownCopyExportBarProps {
  /** Markdown (or HTML) source copied / exported. */
  content: string;
  /** Basename without extension (title-derived). */
  fileBaseName?: string;
  /** When false, hide PDF (e.g. presentInteractive HTML mode). */
  allowPdf?: boolean;
  className?: string;
  /** Align controls to the end (reportResult) or start. */
  align?: 'start' | 'end';
}

/**
 * Shared Copy + Export (Markdown / PDF) controls for presentInteractive
 * parity surfaces like ReportResultCard.
 */
export const MarkdownCopyExportBar = memo(function MarkdownCopyExportBar({
  content,
  fileBaseName,
  allowPdf = true,
  className,
  align = 'end',
}: MarkdownCopyExportBarProps) {
  const { t } = useTranslation('common');
  const isDark = useIsDarkMode();
  const copyingRef = useRef(false);
  const copiedResetTimeoutRef = useRef<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [busyKind, setBusyKind] = useState<MarkdownExportKind | null>(null);

  useEffect(() => {
    return () => {
      if (copiedResetTimeoutRef.current !== null) {
        window.clearTimeout(copiedResetTimeoutRef.current);
      }
    };
  }, []);

  const trimmed = content.trim();
  const disabled = !trimmed || busyKind !== null;
  const baseName = markdownExportBaseName(fileBaseName, 'export');

  const handleCopy = useCallback(async () => {
    if (!trimmed || copyingRef.current) {
      return;
    }
    copyingRef.current = true;
    try {
      await navigator.clipboard.writeText(trimmed.endsWith('\n') ? trimmed : `${trimmed}\n`);
      setCopied(true);
      toast.success(
        t('agent.toolStructured.contentCopied', 'Copied to clipboard'),
      );
      if (copiedResetTimeoutRef.current !== null) {
        window.clearTimeout(copiedResetTimeoutRef.current);
      }
      copiedResetTimeoutRef.current = window.setTimeout(() => {
        copiedResetTimeoutRef.current = null;
        setCopied(false);
      }, COPY_FEEDBACK_MS);
    } catch (error) {
      logger.error('Failed to copy markdown document', error);
      toast.error(
        t('agent.toolStructured.copyResultError', 'Failed to copy result'),
      );
    } finally {
      copyingRef.current = false;
    }
  }, [t, trimmed]);

  const handleExport = useCallback(
    async (kind: MarkdownExportKind) => {
      if (!trimmed || busyKind) {
        return;
      }
      setBusyKind(kind);
      try {
        await exportMarkdownDocumentWithNotify({
          content: trimmed.endsWith('\n') ? trimmed : `${trimmed}\n`,
          kind,
          fileBaseName: baseName,
          isDark,
          labels: markdownExportLabelsFromT(t),
        });
      } catch {
        // Toast already shown in helper.
      } finally {
        setBusyKind(null);
      }
    },
    [baseName, busyKind, isDark, t, trimmed],
  );

  return (
    <div
      className={cn(
        'flex items-center gap-0.5',
        align === 'end' ? 'justify-end' : 'justify-start',
        className,
      )}
      data-testid="markdown-copy-export-bar"
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs text-muted-foreground hover:text-foreground"
            onClick={() => {
              void handleCopy();
            }}
            disabled={disabled}
            data-testid="markdown-copy-button"
            aria-label={t(
              'agent.toolStructured.copyResultAria',
              'Copy result',
            )}
          >
            {copied ? (
              <Check className="mr-1 h-3.5 w-3.5" />
            ) : (
              <Copy className="mr-1 h-3.5 w-3.5" />
            )}
            {t('agent.toolStructured.copyResult', 'Copy')}
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t('agent.toolStructured.copyResult', 'Copy')}
        </TooltipContent>
      </Tooltip>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 gap-0.5 px-1.5 text-muted-foreground hover:text-foreground"
                disabled={disabled}
                aria-label={t('agent.bubble.actionBar.exportAria')}
                data-testid="markdown-export-trigger"
              >
                {busyKind ? (
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
        <DropdownMenuContent align="end" className="min-w-[11rem]">
          <DropdownMenuItem
            data-testid="markdown-export-md"
            onSelect={() => {
              void handleExport('markdown');
            }}
          >
            <FileDown className="h-4 w-4" />
            {t('agent.bubble.actionBar.exportMarkdown')}
          </DropdownMenuItem>
          {allowPdf ? (
            <DropdownMenuItem
              data-testid="markdown-export-pdf"
              onSelect={() => {
                void handleExport('pdf');
              }}
            >
              <Printer className="h-4 w-4" />
              {t('agent.bubble.actionBar.exportPdf')}
            </DropdownMenuItem>
          ) : null}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
});
