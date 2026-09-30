import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  PDF_EMBED_MARKER_PREFIX,
  prepareMarkdownForPdfExport,
} from '../pdf-export-preprocess';

const mockRenderMermaidSvg = vi.fn();
const mockRasterizeSvg = vi.fn();

vi.mock('@/lib/mermaid/loader', () => ({
  renderMermaidSvg: (...args: unknown[]) => mockRenderMermaidSvg(...args),
}));

describe('prepareMarkdownForPdfExport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockRenderMermaidSvg.mockResolvedValue(
      '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"></svg>',
    );
    mockRasterizeSvg.mockResolvedValue(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
    );
  });

  it('leaves markdown without mermaid fences unchanged', async () => {
    const md = '## Answer\n\nInline $E=mc^2$\n';
    const result = await prepareMarkdownForPdfExport(md);
    expect(result.content).toBe(md);
    expect(result.embeddedImages).toEqual([]);
    expect(mockRenderMermaidSvg).not.toHaveBeenCalled();
  });

  it('replaces mermaid fences with embed markers and PNG payloads', async () => {
    const md = [
      'Intro',
      '',
      '```mermaid',
      'flowchart LR',
      '  A --> B',
      '```',
      '',
      'Done',
    ].join('\n');

    const result = await prepareMarkdownForPdfExport(md, {
      rasterizeSvg: mockRasterizeSvg,
    });

    expect(mockRenderMermaidSvg).toHaveBeenCalledWith(
      'flowchart LR\n  A --> B\n',
      false,
    );
    expect(mockRasterizeSvg).toHaveBeenCalledOnce();
    expect(result.embeddedImages).toHaveLength(1);
    expect(result.embeddedImages[0]?.dataBase64.length).toBeGreaterThan(10);
    expect(result.content).toContain(
      `![Mermaid diagram](${PDF_EMBED_MARKER_PREFIX}0)`,
    );
    expect(result.content).not.toContain('```mermaid');
    expect(result.content).toContain('Intro');
    expect(result.content).toContain('Done');
  });

  it('keeps the source fence when mermaid render fails', async () => {
    mockRenderMermaidSvg.mockRejectedValueOnce(new Error('parse failed'));
    const md = '```mermaid\nbad\n```\n';

    const result = await prepareMarkdownForPdfExport(md, {
      rasterizeSvg: mockRasterizeSvg,
    });

    expect(result.embeddedImages).toEqual([]);
    expect(result.content).toBe(md);
    expect(mockRasterizeSvg).not.toHaveBeenCalled();
  });
});
