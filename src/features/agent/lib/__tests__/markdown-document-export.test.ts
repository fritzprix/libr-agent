import { describe, expect, it, vi, beforeEach } from 'vitest';

const mockDownloadTextFile = vi.fn();
const mockDownloadTextPdf = vi.fn();
const mockPrepare = vi.fn();

vi.mock('@/lib/backend', () => ({
  downloadTextFile: (...args: unknown[]) => mockDownloadTextFile(...args),
  downloadTextPdf: (...args: unknown[]) => mockDownloadTextPdf(...args),
}));

vi.mock('@/features/agent/lib/pdf-export-preprocess', () => ({
  prepareMarkdownForPdfExport: (...args: unknown[]) => mockPrepare(...args),
}));

import {
  composeReportResultMarkdown,
  exportMarkdownDocument,
  markdownExportBaseName,
} from '../markdown-document-export';

describe('composeReportResultMarkdown', () => {
  it('returns result only when no title/criteria/proof', () => {
    expect(
      composeReportResultMarkdown({
        result: 'Done.',
      }),
    ).toBe('Done.\n');
  });

  it('includes title, criteria, proof, and outcome sections', () => {
    const md = composeReportResultMarkdown({
      title: 'Ship report',
      criteria: 'Tests pass',
      proof: 'ci green',
      result: 'Released v1',
    });
    expect(md).toContain('# Ship report');
    expect(md).toContain('## Acceptance criteria');
    expect(md).toContain('Tests pass');
    expect(md).toContain('## Verification proof');
    expect(md).toContain('ci green');
    expect(md).toContain('## Outcome');
    expect(md).toContain('Released v1');
  });
});

describe('markdownExportBaseName', () => {
  it('sanitizes unsafe filename characters', () => {
    expect(markdownExportBaseName('A/B:C?.md')).toBe('A_B_C_.md');
  });

  it('falls back when empty', () => {
    expect(markdownExportBaseName('  ', 'result')).toBe('result');
  });
});

describe('exportMarkdownDocument', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDownloadTextFile.mockResolvedValue('/tmp/export.md');
    mockDownloadTextPdf.mockResolvedValue('/tmp/export.pdf');
    mockPrepare.mockResolvedValue({
      content: 'prepared',
      embeddedImages: [{ dataBase64: 'aaa' }],
    });
  });

  it('writes markdown via downloadTextFile', async () => {
    await exportMarkdownDocument({
      content: '# Hi\n',
      kind: 'markdown',
      fileBaseName: 'note',
    });
    expect(mockDownloadTextFile).toHaveBeenCalledWith({
      fileName: 'note.md',
      content: '# Hi\n',
    });
  });

  it('preprocesses then writes PDF', async () => {
    await exportMarkdownDocument({
      content: '```mermaid\nA-->B\n```\n',
      kind: 'pdf',
      fileBaseName: 'diagram',
      isDark: true,
    });
    expect(mockPrepare).toHaveBeenCalledWith('```mermaid\nA-->B\n```\n', {
      isDark: true,
    });
    expect(mockDownloadTextPdf).toHaveBeenCalledWith({
      fileName: 'diagram.pdf',
      content: 'prepared',
      embeddedImages: [{ dataBase64: 'aaa' }],
    });
  });
});
