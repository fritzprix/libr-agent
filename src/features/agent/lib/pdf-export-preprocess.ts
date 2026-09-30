import { renderMermaidSvg } from '@/lib/mermaid/loader';
import { getLogger } from '@/lib/logger';

const logger = getLogger('pdf-export-preprocess');

/** Marker URL rewritten by Rust to a temp PNG path before markdown2pdf. */
export const PDF_EMBED_MARKER_PREFIX = 'libragent-pdf-embed:';

const MERMAID_FENCE_RE = /^```mermaid[^\n]*\r?\n([\s\S]*?)^```[ \t]*$/gim;

/** Matches MermaidBlock / chat canvas chrome backgrounds. */
const PDF_LIGHT_BG = '#ffffff';
const PDF_DARK_BG = '#0b1220';

export interface PdfEmbeddedImagePayload {
  dataBase64: string;
}

export interface PreparedPdfMarkdown {
  content: string;
  embeddedImages: PdfEmbeddedImagePayload[];
}

export interface PrepareMarkdownForPdfOptions {
  /**
   * Match chat Mermaid theme (`useIsDarkMode()`). Defaults to light so PDF
   * pages stay readable when the caller does not pass a theme.
   */
  isDark?: boolean;
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
  const isDark = Boolean(options.isDark);
  const rasterizeSvg =
    options.rasterizeSvg ??
    ((svg: string) => svgToPngBase64(svg, { isDark }));
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
    // Match CodeBlock: drop a single trailing newline before trim-in-loader.
    const code = String(match[1] ?? '').replace(/\n$/, '');
    const index = match.index ?? 0;
    out += markdown.slice(lastIndex, index);

    try {
      // Same theme path as MermaidBlock / chat messages.
      const svg = await renderMermaidSvg(code, isDark);
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

export interface SvgToPngOptions {
  isDark?: boolean;
  /** Device-pixel scale for sharper embeds (default 2). */
  scale?: number;
}

/**
 * Rasterize an SVG string to PNG base64 (no data: prefix).
 * Normalizes % / missing dimensions from Mermaid so `<img>` decoding works —
 * the common failure mode when chat SVG is fine but PDF falls back to fences.
 */
export async function svgToPngBase64(
  svg: string,
  options: SvgToPngOptions = {},
): Promise<string> {
  const trimmed = svg.trim();
  if (!trimmed) {
    throw new Error('empty SVG');
  }

  const isDark = Boolean(options.isDark);
  const scale = Math.max(1, options.scale ?? 2);
  const background = isDark ? PDF_DARK_BG : PDF_LIGHT_BG;
  const prepared = normalizeSvgForRaster(trimmed);

  try {
    return await rasterSvgViaImage(prepared, scale, background);
  } catch (imageError) {
    logger.warn(
      'SVG Image raster failed; trying DOM measurement fallback',
      imageError,
    );
    return await rasterSvgViaDom(prepared.svg, prepared.width, prepared.height, scale, background);
  }
}

interface NormalizedSvg {
  svg: string;
  width: number;
  height: number;
}

/** Force absolute px size + xmlns so browsers can decode Mermaid SVG as an image. */
export function normalizeSvgForRaster(svg: string): NormalizedSvg {
  const parser = new DOMParser();
  const doc = parser.parseFromString(svg, 'image/svg+xml');
  const root = doc.documentElement;
  if (
    !root ||
    root.nodeName.toLowerCase() === 'parsererror' ||
    root.querySelector('parsererror')
  ) {
    throw new Error('invalid SVG markup');
  }

  const widthAttr = root.getAttribute('width');
  const heightAttr = root.getAttribute('height');
  let width = parseSvgLength(widthAttr);
  let height = parseSvgLength(heightAttr);

  const viewBox = root.getAttribute('viewBox');
  if ((!width || !height) && viewBox) {
    const parts = viewBox
      .trim()
      .split(/[\s,]+/)
      .map((part) => Number(part));
    if (parts.length === 4 && parts.every((n) => Number.isFinite(n))) {
      width = width ?? Math.max(1, parts[2]);
      height = height ?? Math.max(1, parts[3]);
    }
  }

  width = Math.max(1, Math.ceil(width ?? 800));
  height = Math.max(1, Math.ceil(height ?? 600));

  root.setAttribute('width', String(width));
  root.setAttribute('height', String(height));
  if (!root.getAttribute('xmlns')) {
    root.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  }

  return {
    svg: new XMLSerializer().serializeToString(root),
    width,
    height,
  };
}

function parseSvgLength(value: string | null): number | null {
  if (!value) {
    return null;
  }
  const trimmed = value.trim();
  if (!trimmed || /%/.test(trimmed) || trimmed === 'auto') {
    return null;
  }
  const numeric = Number.parseFloat(trimmed);
  return Number.isFinite(numeric) && numeric > 0 ? numeric : null;
}

async function rasterSvgViaImage(
  prepared: NormalizedSvg,
  scale: number,
  background: string,
): Promise<string> {
  // data: URLs are more reliable than blob: for SVG+XML across Chromium/WebView.
  const dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(prepared.svg)}`;
  const image = await loadImage(dataUrl);
  return drawImageToPngBase64(image, prepared.width, prepared.height, scale, background);
}

async function rasterSvgViaDom(
  svgMarkup: string,
  fallbackWidth: number,
  fallbackHeight: number,
  scale: number,
  background: string,
): Promise<string> {
  const host = document.createElement('div');
  host.setAttribute('data-pdf-mermaid-raster', 'true');
  host.style.cssText =
    'position:fixed;left:-10000px;top:0;width:auto;height:auto;opacity:0;pointer-events:none;';
  host.innerHTML = svgMarkup;
  document.body.appendChild(host);

  try {
    const svgEl = host.querySelector('svg');
    if (!svgEl) {
      throw new Error('DOM raster: svg element missing');
    }

    let width = fallbackWidth;
    let height = fallbackHeight;
    try {
      const box = svgEl.getBBox();
      if (box.width > 0 && box.height > 0) {
        width = Math.max(1, Math.ceil(box.width));
        height = Math.max(1, Math.ceil(box.height));
        svgEl.setAttribute('width', String(width));
        svgEl.setAttribute('height', String(height));
      }
    } catch {
      // getBBox can throw if the SVG is not rendered; keep normalized size.
    }

    const serialized = new XMLSerializer().serializeToString(svgEl);
    const dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(serialized)}`;
    const image = await loadImage(dataUrl);
    return drawImageToPngBase64(image, width, height, scale, background);
  } finally {
    host.remove();
  }
}

function drawImageToPngBase64(
  image: HTMLImageElement,
  width: number,
  height: number,
  scale: number,
  background: string,
): string {
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.ceil(width * scale));
  canvas.height = Math.max(1, Math.ceil(height * scale));
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    throw new Error('canvas 2d context unavailable');
  }
  ctx.fillStyle = background;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(image, 0, 0, canvas.width, canvas.height);

  const dataUrl = canvas.toDataURL('image/png');
  const comma = dataUrl.indexOf(',');
  if (comma < 0) {
    throw new Error('invalid PNG data URL');
  }
  return dataUrl.slice(comma + 1);
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('failed to decode SVG as image'));
    image.src = src;
  });
}
