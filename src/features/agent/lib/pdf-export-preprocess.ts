import { renderMermaidSvg } from '@/lib/mermaid/loader';
import { getLogger } from '@/lib/logger';

const logger = getLogger('pdf-export-preprocess');

/** Marker URL rewritten by Rust to a temp PNG path before markdown2pdf. */
export const PDF_EMBED_MARKER_PREFIX = 'libragent-pdf-embed:';

const MERMAID_FENCE_RE = /^```mermaid[^\n]*\r?\n([\s\S]*?)^```[ \t]*$/gim;

export interface PdfEmbeddedImagePayload {
  dataBase64: string;
}

export interface PreparedPdfMarkdown {
  content: string;
  embeddedImages: PdfEmbeddedImagePayload[];
}

export interface PrepareMarkdownForPdfOptions {
  /** Override SVG→PNG for tests; defaults to canvas rasterization. */
  rasterizeSvg?: (svg: string) => Promise<string>;
}

/**
 * Replace ```mermaid fences with PNG embed markers for PDF export.
 * Failed diagrams keep the original fence (source fallback).
 * LaTeX `$` / `$$` is left for markdown2pdf's built-in TeX engine.
 */
export async function prepareMarkdownForPdfExport(
  markdown: string,
  options: PrepareMarkdownForPdfOptions = {},
): Promise<PreparedPdfMarkdown> {
  const rasterizeSvg = options.rasterizeSvg ?? svgToPngBase64;
  const embeddedImages: PdfEmbeddedImagePayload[] = [];
  // Clone so the module-level `/g` regex lastIndex cannot leak across calls.
  const fenceRe = new RegExp(MERMAID_FENCE_RE.source, MERMAID_FENCE_RE.flags);
  const matches = [...markdown.matchAll(fenceRe)];
  if (matches.length === 0) {
    return { content: markdown, embeddedImages };
  }

  let out = '';
  let lastIndex = 0;

  for (const match of matches) {
    const full = match[0];
    const code = match[1] ?? '';
    const index = match.index ?? 0;
    out += markdown.slice(lastIndex, index);

    try {
      const svg = await renderMermaidSvg(code, false);
      const dataBase64 = await rasterizeSvg(svg);
      const embedIndex = embeddedImages.length;
      embeddedImages.push({ dataBase64 });
      out += `![Mermaid diagram](${PDF_EMBED_MARKER_PREFIX}${embedIndex})\n`;
    } catch (error) {
      logger.warn('Mermaid PDF preprocess failed; keeping source fence', error);
      out += full;
    }

    lastIndex = index + full.length;
  }

  out += markdown.slice(lastIndex);
  return { content: out, embeddedImages };
}

/** Rasterize an SVG string to PNG base64 (no data: prefix). */
export async function svgToPngBase64(svg: string): Promise<string> {
  const trimmed = svg.trim();
  if (!trimmed) {
    throw new Error('empty SVG');
  }

  const blob = new Blob([trimmed], { type: 'image/svg+xml;charset=utf-8' });
  const objectUrl = URL.createObjectURL(blob);

  try {
    const image = await loadImage(objectUrl);
    const width = Math.max(1, Math.ceil(image.naturalWidth || image.width || 800));
    const height = Math.max(
      1,
      Math.ceil(image.naturalHeight || image.height || 600),
    );

    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('canvas 2d context unavailable');
    }
    // Opaque white — PDF github theme is light; transparent PNG can look black.
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0, 0, width, height);
    ctx.drawImage(image, 0, 0, width, height);

    const dataUrl = canvas.toDataURL('image/png');
    const comma = dataUrl.indexOf(',');
    if (comma < 0) {
      throw new Error('invalid PNG data URL');
    }
    return dataUrl.slice(comma + 1);
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('failed to decode SVG as image'));
    image.src = src;
  });
}
