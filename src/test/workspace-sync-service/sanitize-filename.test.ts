import { describe, expect, it } from 'vitest';
import { sanitizeFilename } from '@/lib/workspace-sync-service';

describe('sanitizeFilename pipeline', () => {
  it('normalizes unicode (NFKC)', () => {
    // calling normalizeUnicode via sanitizeFilename indirectly - ensure no throw
    const out = sanitizeFilename('café.txt');
    expect(out.endsWith('.txt')).toBe(true);
  });

  it('replaces unsafe characters and collapses whitespace', () => {
    const input = 'my<unsafe> name?.md';
    const out = sanitizeFilename(input);
    expect(out).not.toContain('<');
    expect(out).not.toContain('?');
    expect(out).not.toContain(' ');
  });

  it('keeps extension and lowercases it', () => {
    const out = sanitizeFilename('Report.PDF');
    expect(out.endsWith('.pdf')).toBe(true);
  });

  it('ensures non-empty base', () => {
    const out = sanitizeFilename('....');
    expect(out).toBe('file');
  });

  it('limits length to 200 chars', () => {
    const long = 'a'.repeat(300) + '.txt';
    const out = sanitizeFilename(long);
    expect(out.length).toBeLessThanOrEqual(200);
  });
});
