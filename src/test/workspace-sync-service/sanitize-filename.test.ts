import { describe, it, expect } from 'vitest';
import { sanitizeFilename } from '@/lib/workspace-sync-service';

describe('sanitizeFilename', () => {
  it('keeps simple names and lowercases extension', () => {
    expect(sanitizeFilename('hello.txt')).toBe('hello.txt');
    expect(sanitizeFilename('hello.TXT')).toBe('hello.txt');
  });

  it('replaces unsafe characters and whitespace with underscores', () => {
    expect(sanitizeFilename('a b/c\\d:e*f?g"h<i>j|k.txt')).toBe(
      'a_b_c_d_e_f_g_h_i_j_k.txt',
    );
  });

  it('collapses multiple underscores and trims', () => {
    expect(sanitizeFilename('  __a__b__ .md  ')).toBe('a_b.md');
  });

  it('handles filenames without extension', () => {
    expect(sanitizeFilename('my file name')).toBe('my_file_name');
  });

  it('sanitizes leading dot and repeated dots in base', () => {
    // Behavior: leading dots in base are collapsed -> trimmed -> non-empty base retained
    expect(sanitizeFilename('...env.local')).toBe('env.local');
    expect(sanitizeFilename('my..config.json')).toBe('my_config.json');
  });

  it('removes invalid characters from extension and lowercases', () => {
    expect(sanitizeFilename('report.F*I#L!E$')).toBe('report.file');
    expect(sanitizeFilename('photo.jp*g')).toBe('photo.jpg');
  });

  it('ensures non-empty base becomes "file"', () => {
    expect(sanitizeFilename('..')).toBe('file');
    expect(sanitizeFilename(' . ')).toBe('file');
  });

  it('limits length to 200 chars', () => {
    const longBase = 'a'.repeat(300);
    const result = sanitizeFilename(`${longBase}.TXT`);
    expect(result.length).toBeLessThanOrEqual(200);
    // Extension may be truncated due to pre-split length limiting; only length guarantee applies
    // Ensure it is not empty and contains only allowed characters
    expect(result).not.toHaveLength(0);
    expect(/^[A-Za-z0-9_.-]+$/.test(result)).toBe(true);
  });

  it('does not treat leading dot as extension separator', () => {
    // Behavior: a single leading dot means no extension; base sanitized to non-empty
    expect(sanitizeFilename('.gitignore')).toBe('gitignore');
  });

  it('normalizes unicode and spaces', () => {
    // Using full-width characters and mixed spaces: NFKC normalizes to ASCII where applicable
    expect(sanitizeFilename('ｔｅｓｔ　ｆｉｌｅ　Ｎａｍｅ．ＰＮＧ')).toBe('test_file_Name.png');
  });
});
