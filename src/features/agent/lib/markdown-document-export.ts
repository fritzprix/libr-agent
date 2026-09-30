import { downloadTextFile, downloadTextPdf } from '@/lib/backend';
import {
  DOWNLOAD_CANCELLED,
  notifyFileDownloadSuccess,
} from '@/lib/notify-file-download';
import { prepareMarkdownForPdfExport } from '@/features/agent/lib/pdf-export-preprocess';
import { getLogger } from '@/lib/logger';

const logger = getLogger('markdown-document-export');

export type MarkdownExportKind = 'markdown' | 'pdf' | 'html';

export interface MarkdownExportLabels {
  markdownSuccess: string;
  pdfSuccess: string;
  htmlSuccess: string;
  openFile: string;
  openFileError: string;
  cancelled: string;
  markdownError: string;
  pdfError: string;
  htmlError: string;
}

function sanitizeFileBase(name: string): string {
  const trimmed = name.trim().replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_');
  return trimmed.slice(0, 80) || 'export';
}

/** Build a download basename without extension. */
export function markdownExportBaseName(
  title: string | null | undefined,
  fallback = 'export',
): string {
  if (!title?.trim()) {
    return fallback;
  }
  return sanitizeFileBase(title);
}

/**
 * Export markdown source as `.md` or Mermaid/LaTeX-aware `.pdf`.
 * Returns the saved path, `DOWNLOAD_CANCELLED`, or throws.
 */
export async function exportMarkdownDocument(options: {
  content: string;
  kind: MarkdownExportKind;
  fileBaseName: string;
  isDark?: boolean;
}): Promise<string> {
  const { content, kind, fileBaseName, isDark = false } = options;
  const base = sanitizeFileBase(fileBaseName);

  if (kind === 'markdown') {
    return downloadTextFile({
      fileName: `${base}.md`,
      content,
    });
  }

  if (kind === 'html') {
    return downloadTextFile({
      fileName: `${base}.html`,
      content,
    });
  }

  const prepared = await prepareMarkdownForPdfExport(content, { isDark });
  return downloadTextPdf({
    fileName: `${base}.pdf`,
    content: prepared.content,
    embeddedImages:
      prepared.embeddedImages.length > 0 ? prepared.embeddedImages : undefined,
  });
}

/** Run export and surface the same toast UX as MessageActionBar. */
export async function exportMarkdownDocumentWithNotify(options: {
  content: string;
  kind: MarkdownExportKind;
  fileBaseName: string;
  isDark?: boolean;
  labels: MarkdownExportLabels;
}): Promise<void> {
  const { content, kind, fileBaseName, isDark, labels } = options;
  try {
    const result = await exportMarkdownDocument({
      content,
      kind,
      fileBaseName,
      isDark,
    });
    if (result === DOWNLOAD_CANCELLED) {
      const { toast } = await import('sonner');
      toast.info(labels.cancelled);
      return;
    }
    notifyFileDownloadSuccess({
      title:
        kind === 'pdf'
          ? labels.pdfSuccess
          : kind === 'html'
            ? labels.htmlSuccess
            : labels.markdownSuccess,
      filePath: result,
      openLabel: labels.openFile,
      openErrorLabel: labels.openFileError,
    });
  } catch (error) {
    logger.error('Markdown document export failed', { kind, error });
    const { toast } = await import('sonner');
    toast.error(
      kind === 'pdf'
        ? labels.pdfError
        : kind === 'html'
          ? labels.htmlError
          : labels.markdownError,
    );
    throw error;
  }
}

/**
 * Compose reportResult fields into one markdown document for copy/export.
 * Matches presentInteractive: one body of markdown source the user can share.
 */
export function composeReportResultMarkdown(data: {
  title?: string | null;
  criteria?: string | null;
  proof?: string | null;
  result: string;
}): string {
  const parts: string[] = [];
  if (data.title?.trim()) {
    parts.push(`# ${data.title.trim()}`, '');
  }
  if (data.criteria?.trim()) {
    parts.push('## Acceptance criteria', '', data.criteria.trim(), '');
  }
  if (data.proof?.trim()) {
    parts.push('## Verification proof', '', data.proof.trim(), '');
  }
  if (data.title?.trim() || data.criteria?.trim() || data.proof?.trim()) {
    parts.push('## Outcome', '', data.result.trim(), '');
  } else {
    parts.push(data.result.trim(), '');
  }
  return parts.join('\n').trimEnd() + '\n';
}

/** i18n labels shared by MessageActionBar / ReportResult / presentInteractive. */
export function markdownExportLabelsFromT(
  t: (key: string) => string,
): MarkdownExportLabels {
  return {
    markdownSuccess: t('agent.bubble.actionBar.exportMarkdownSuccess'),
    pdfSuccess: t('agent.bubble.actionBar.exportPdfSuccess'),
    htmlSuccess: t('agent.bubble.actionBar.exportHtmlSuccess'),
    openFile: t('agent.bubble.actionBar.exportOpenFile'),
    openFileError: t('agent.bubble.actionBar.exportOpenFileError'),
    cancelled: t('agent.bubble.actionBar.exportCancelled'),
    markdownError: t('agent.bubble.actionBar.exportError'),
    pdfError: t('agent.bubble.actionBar.exportPdfError'),
    htmlError: t('agent.bubble.actionBar.exportHtmlError'),
  };
}
