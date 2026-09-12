import { describe, expect, it } from 'vitest';
import { classifyReportResultLink } from '../reportResultLinks';

describe('classifyReportResultLink', () => {
  it('classifies http(s) as external and blocks javascript', () => {
    expect(classifyReportResultLink('https://example.com/a')).toEqual({
      kind: 'external',
      url: 'https://example.com/a',
    });
    expect(classifyReportResultLink('javascript:alert(1)')).toEqual({
      kind: 'blocked',
    });
    expect(classifyReportResultLink('data:text/html,hi')).toEqual({
      kind: 'blocked',
    });
  });

  it('keeps Windows drive paths as host opens', () => {
    expect(classifyReportResultLink('C:\\Users\\a\\out.md')).toEqual({
      kind: 'host',
      absolutePath: 'C:\\Users\\a\\out.md',
    });
  });

  it('classifies workspace-relative paths and ./ prefixes', () => {
    expect(classifyReportResultLink('outputs/report.pdf')).toEqual({
      kind: 'workspace',
      path: 'outputs/report.pdf',
    });
    expect(classifyReportResultLink('./notes.md')).toEqual({
      kind: 'workspace',
      path: 'notes.md',
    });
    expect(classifyReportResultLink('/workspace/data/out.csv')).toEqual({
      kind: 'workspace',
      path: 'data/out.csv',
    });
  });

  it('blocks spa-like root paths that are not workspace files', () => {
    // Absolute host path — opened via OS, not SPA router
    expect(classifyReportResultLink('/tmp/out.md')).toEqual({
      kind: 'host',
      absolutePath: '/tmp/out.md',
    });
  });

  it('prefers matching deliverable absolute paths', () => {
    expect(
      classifyReportResultLink('dist/app.zip', [
        {
          path: 'dist/app.zip',
          absolute_path: '/abs/dist/app.zip',
          name: 'app.zip',
          exists: true,
        },
      ]),
    ).toEqual({
      kind: 'host',
      absolutePath: '/abs/dist/app.zip',
    });
  });

  it('blocks empty and hash-only hrefs', () => {
    expect(classifyReportResultLink(undefined)).toEqual({ kind: 'blocked' });
    expect(classifyReportResultLink('#section')).toEqual({ kind: 'blocked' });
  });
});
