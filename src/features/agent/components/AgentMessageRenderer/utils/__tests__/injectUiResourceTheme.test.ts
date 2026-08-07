import { describe, expect, it, vi } from 'vitest';
import {
  DARK_UI_RESOURCE_THEME_VARS,
  UI_RESOURCE_THEME_MARKER,
  applyThemeToUiResource,
  buildUiResourceThemeStyleTag,
  collectThemeCssVariables,
  getUiResourceThemeVars,
  injectUiResourceThemeHtml,
} from '../injectUiResourceTheme';

describe('injectUiResourceTheme', () => {
  it('getUiResourceThemeVars returns static dark tokens without DOM', () => {
    expect(getUiResourceThemeVars(true)['--background']).toBe(
      DARK_UI_RESOURCE_THEME_VARS['--background'],
    );
    expect(getUiResourceThemeVars(false)['--background']).toBe('oklch(1 0 0)');
  });

  it('builds a style tag with color-scheme and provided tokens', () => {
    const tag = buildUiResourceThemeStyleTag(true, {
      '--background': 'oklch(0.145 0 0)',
      '--foreground': 'oklch(0.985 0 0)',
    });

    expect(tag).toContain(UI_RESOURCE_THEME_MARKER);
    expect(tag).toContain('color-scheme: dark');
    expect(tag).toContain('--background: oklch(0.145 0 0)');
    expect(tag).toContain('--foreground: oklch(0.985 0 0)');
    expect(tag).toContain('background-color: var(--background');
  });

  it('injects before </body> and replaces a previous theme block', () => {
    const styleTag = buildUiResourceThemeStyleTag(false, {
      '--background': 'white',
    });
    const html = `<html><head><title>x</title></head><body><p>hi</p></body></html>`;
    const once = injectUiResourceThemeHtml(html, styleTag);
    expect(once).toMatch(
      /<p>hi<\/p><style data-libragent-theme="true">[\s\S]*<\/style><\/body>/,
    );

    const darkTag = buildUiResourceThemeStyleTag(true, {
      '--background': 'black',
    });
    const twice = injectUiResourceThemeHtml(once, darkTag);
    expect(twice.match(new RegExp(UI_RESOURCE_THEME_MARKER, 'g'))).toHaveLength(
      1,
    );
    expect(twice).toContain('color-scheme: dark');
    expect(twice).not.toContain('color-scheme: light');
  });

  it('appends when there is no body/html closer', () => {
    const styleTag = buildUiResourceThemeStyleTag(true, {});
    const result = injectUiResourceThemeHtml('<div>widget</div>', styleTag);
    expect(result.startsWith('<div>widget</div>')).toBe(true);
    expect(result).toContain(`<style ${UI_RESOURCE_THEME_MARKER}`);
  });

  it('overrides hardcoded light widget chrome for stored reportResult HTML', () => {
    const styleTag = buildUiResourceThemeStyleTag(
      true,
      getUiResourceThemeVars(true),
    );
    const widget = `<!doctype html><html><head><style>
      body { background: #f9fafb; color: #1f2937; }
      .content-wrapper { background: white; color: #111827; }
    </style></head><body><div class="content-wrapper">x</div></body></html>`;

    const themed = injectUiResourceThemeHtml(widget, styleTag);
    expect(themed.indexOf('background: white')).toBeLessThan(
      themed.indexOf('data-libragent-theme'),
    );
    expect(themed).toContain('.content-wrapper');
    expect(themed).toContain('background: var(--card');
    expect(themed).toContain('color-scheme: dark');
  });

  it('applyThemeToUiResource only mutates text/html with text', () => {
    const styleTag = buildUiResourceThemeStyleTag(true, {
      '--background': 'oklch(0.1 0 0)',
    });

    const htmlResource = {
      uri: 'ui://x',
      mimeType: 'text/html',
      text: '<div>a</div>',
    };
    const themed = applyThemeToUiResource(htmlResource, styleTag);
    expect(themed).not.toBe(htmlResource);
    expect(themed.text).toContain(UI_RESOURCE_THEME_MARKER);

    const urlResource = {
      uri: 'ui://y',
      mimeType: 'text/uri-list',
      text: 'https://example.com',
    };
    expect(applyThemeToUiResource(urlResource, styleTag)).toBe(urlResource);

    const empty = {
      uri: 'ui://z',
      mimeType: 'text/html',
      text: '',
    };
    expect(applyThemeToUiResource(empty, styleTag)).toBe(empty);

    expect(applyThemeToUiResource(htmlResource, null)).toBe(htmlResource);
  });

  it('collectThemeCssVariables reads computed style tokens', () => {
    const getPropertyValue = vi.fn((name: string) => {
      if (name === '--background') return '  oklch(0.2 0 0)  ';
      if (name === '--foreground') return 'oklch(0.9 0 0)';
      return '';
    });
    const root = document.createElement('div');

    const vars = collectThemeCssVariables(root, () => ({ getPropertyValue }));
    expect(vars).toEqual({
      '--background': 'oklch(0.2 0 0)',
      '--foreground': 'oklch(0.9 0 0)',
    });
    expect(getPropertyValue).toHaveBeenCalled();
  });
});
