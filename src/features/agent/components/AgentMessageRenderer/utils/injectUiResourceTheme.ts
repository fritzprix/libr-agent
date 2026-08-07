/**
 * Host-side theme injection for MCP UI rawHtml resources.
 *
 * rawHtml iframes use `sandbox="allow-scripts"` without `allow-same-origin`,
 * so the parent cannot touch `contentDocument`. Appending a style block into
 * the HTML string (before `</body>` / `</html>`) is the reliable path for
 * uncooperative widgets — including ones that hardcode light colors.
 */

export const UI_RESOURCE_THEME_MARKER = 'data-libragent-theme';

const THEME_STYLE_RE = new RegExp(
  `<style\\b[^>]*\\b${UI_RESOURCE_THEME_MARKER}\\b[^>]*>[\\s\\S]*?<\\/style>`,
  'i',
);

/** Semantic tokens mirrored into the iframe document. */
export const UI_RESOURCE_THEME_TOKENS = [
  '--background',
  '--foreground',
  '--card',
  '--card-foreground',
  '--popover',
  '--popover-foreground',
  '--primary',
  '--primary-foreground',
  '--secondary',
  '--secondary-foreground',
  '--muted',
  '--muted-foreground',
  '--accent',
  '--accent-foreground',
  '--destructive',
  '--border',
  '--input',
  '--ring',
  '--radius',
] as const;

export type UiResourceThemeToken = (typeof UI_RESOURCE_THEME_TOKENS)[number];

/**
 * Static tokens mirrored from `src/styles/globals.css`.
 * Prefer these over `getComputedStyle` at render time: next-themes can report
 * dark while `<html>` still lacks `.dark`, which would bake light tokens into
 * the iframe permanently.
 */
export const LIGHT_UI_RESOURCE_THEME_VARS: Record<
  UiResourceThemeToken,
  string
> = {
  '--background': 'oklch(1 0 0)',
  '--foreground': 'oklch(0.145 0 0)',
  '--card': 'oklch(1 0 0)',
  '--card-foreground': 'oklch(0.145 0 0)',
  '--popover': 'oklch(1 0 0)',
  '--popover-foreground': 'oklch(0.145 0 0)',
  '--primary': 'oklch(0.205 0 0)',
  '--primary-foreground': 'oklch(0.985 0 0)',
  '--secondary': 'oklch(0.97 0 0)',
  '--secondary-foreground': 'oklch(0.205 0 0)',
  '--muted': 'oklch(0.97 0 0)',
  '--muted-foreground': 'oklch(0.556 0 0)',
  '--accent': 'oklch(0.97 0 0)',
  '--accent-foreground': 'oklch(0.205 0 0)',
  '--destructive': 'oklch(0.577 0.245 27.325)',
  '--border': 'oklch(0.922 0 0)',
  '--input': 'oklch(0.922 0 0)',
  '--ring': 'oklch(0.708 0 0)',
  '--radius': '0.625rem',
};

export const DARK_UI_RESOURCE_THEME_VARS: Record<
  UiResourceThemeToken,
  string
> = {
  '--background': 'oklch(0.145 0 0)',
  '--foreground': 'oklch(0.985 0 0)',
  '--card': 'oklch(0.205 0 0)',
  '--card-foreground': 'oklch(0.985 0 0)',
  '--popover': 'oklch(0.205 0 0)',
  '--popover-foreground': 'oklch(0.985 0 0)',
  '--primary': 'oklch(0.922 0 0)',
  '--primary-foreground': 'oklch(0.205 0 0)',
  '--secondary': 'oklch(0.269 0 0)',
  '--secondary-foreground': 'oklch(0.985 0 0)',
  '--muted': 'oklch(0.269 0 0)',
  '--muted-foreground': 'oklch(0.708 0 0)',
  '--accent': 'oklch(0.269 0 0)',
  '--accent-foreground': 'oklch(0.985 0 0)',
  '--destructive': 'oklch(0.704 0.191 22.216)',
  '--border': 'oklch(1 0 0 / 10%)',
  '--input': 'oklch(1 0 0 / 15%)',
  '--ring': 'oklch(0.556 0 0)',
  '--radius': '0.625rem',
};

/** Minimal style surface needed to read CSS custom properties. */
export type CssCustomPropertyReader = {
  getPropertyValue(property: string): string;
};

export type UiResourceLike = {
  uri?: string;
  mimeType?: string;
  text?: string;
  blob?: string;
  _meta?: Record<string, unknown>;
};

export function getUiResourceThemeVars(
  isDark: boolean,
): Record<UiResourceThemeToken, string> {
  return isDark ? DARK_UI_RESOURCE_THEME_VARS : LIGHT_UI_RESOURCE_THEME_VARS;
}

export function collectThemeCssVariables(
  root: Element | null = typeof document !== 'undefined'
    ? document.documentElement
    : null,
  readStyle: (element: Element) => CssCustomPropertyReader = (element) =>
    getComputedStyle(element),
): Partial<Record<UiResourceThemeToken, string>> {
  if (!root) {
    return {};
  }

  const styles = readStyle(root);
  const vars: Partial<Record<UiResourceThemeToken, string>> = {};

  for (const token of UI_RESOURCE_THEME_TOKENS) {
    const value = styles.getPropertyValue(token).trim();
    if (value) {
      vars[token] = value;
    }
  }

  return vars;
}

export function buildUiResourceThemeStyleTag(
  isDark: boolean,
  vars: Partial<Record<UiResourceThemeToken, string>>,
): string {
  const declarations = UI_RESOURCE_THEME_TOKENS.flatMap((token) => {
    const value = vars[token];
    return value ? [`  ${token}: ${value};`] : [];
  }).join('\n');

  const colorScheme = isDark ? 'dark' : 'light';

  // Appended after widget CSS so these rules win over hardcoded light colors
  // in builtin templates (and older stored tool results).
  return `<style ${UI_RESOURCE_THEME_MARKER}="true">
:root {
  color-scheme: ${colorScheme};
${declarations}
}
html, body {
  background-color: var(--background, ${isDark ? '#0a0a0a' : '#ffffff'}) !important;
  color: var(--foreground, ${isDark ? '#fafafa' : '#0a0a0a'}) !important;
  margin: 0;
}
.content-wrapper,
.interaction-wrapper,
.circuit-container {
  background: var(--card, ${isDark ? '#1a1a1a' : '#ffffff'}) !important;
  color: var(--card-foreground, ${isDark ? '#fafafa' : '#111827'}) !important;
  border-color: var(--border, ${isDark ? 'rgba(255,255,255,0.1)' : '#e5e7eb'}) !important;
  box-shadow: 0 1px 3px color-mix(in oklab, var(--foreground, #000) 12%, transparent) !important;
}
.circuit-container {
  border-color: #f59e0b !important;
}
.tool-info {
  background: color-mix(in oklab, #f59e0b 16%, var(--muted)) !important;
  color: var(--foreground) !important;
}
.content-title,
.interaction-prompt,
.md h1,
.md h2 {
  color: var(--card-foreground, inherit) !important;
  border-bottom-color: var(--border, currentColor) !important;
}
.md h3,
.option-label {
  color: var(--muted-foreground, inherit) !important;
}
.icon-btn {
  background: var(--muted) !important;
  color: var(--muted-foreground) !important;
  border-color: var(--border) !important;
}
.icon-btn:hover {
  background: var(--accent) !important;
  color: var(--accent-foreground) !important;
}
.option-item {
  border-color: var(--border) !important;
}
.option-item:hover {
  background: var(--muted) !important;
}
.text-input {
  background: var(--background) !important;
  color: var(--foreground) !important;
  border-color: var(--input) !important;
}
.btn-secondary {
  background: var(--secondary) !important;
  color: var(--secondary-foreground) !important;
}
.md code,
.md th {
  background: var(--muted) !important;
}
.md th,
.md td {
  border-color: var(--border) !important;
}
</style>`;
}

/**
 * Strip a previous LibrAgent theme block (if any) and insert `styleTag`
 * before `</body>` / `</html>` (or at the end) so it overrides widget CSS.
 */
export function injectUiResourceThemeHtml(
  html: string,
  styleTag: string,
): string {
  const withoutTheme = html.replace(THEME_STYLE_RE, '');
  const bodyClose = withoutTheme.match(/<\/body>/i);
  if (bodyClose && bodyClose.index !== undefined) {
    return (
      withoutTheme.slice(0, bodyClose.index) +
      styleTag +
      withoutTheme.slice(bodyClose.index)
    );
  }

  const htmlClose = withoutTheme.match(/<\/html>/i);
  if (htmlClose && htmlClose.index !== undefined) {
    return (
      withoutTheme.slice(0, htmlClose.index) +
      styleTag +
      withoutTheme.slice(htmlClose.index)
    );
  }

  return withoutTheme + styleTag;
}

function isRawHtmlMimeType(mimeType: string | undefined): boolean {
  if (!mimeType) return false;
  return mimeType.split(';')[0]?.trim().toLowerCase() === 'text/html';
}

/**
 * Returns a shallow-copied resource with theme CSS appended into `text`
 * for rawHtml (`text/html`) only. Non-HTML / textless resources are returned
 * unchanged (same reference).
 */
export function applyThemeToUiResource<T extends UiResourceLike>(
  resource: T,
  styleTag: string | null | undefined,
): T {
  if (!styleTag || !isRawHtmlMimeType(resource.mimeType)) {
    return resource;
  }

  if (typeof resource.text !== 'string' || resource.text.length === 0) {
    return resource;
  }

  const nextText = injectUiResourceThemeHtml(resource.text, styleTag);
  if (nextText === resource.text) {
    return resource;
  }

  return {
    ...resource,
    text: nextText,
  };
}
