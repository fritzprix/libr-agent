import { describe, expect, it } from 'vitest';

import {
  formatDuration,
  pruneRunDirectories,
  sanitizeStageName,
  summarizeFailureText,
  tailLines,
} from '../refactor-pipeline.js';

describe('refactor-pipeline helpers', () => {
  it('sanitizes stage names for filesystem-safe log files', () => {
    expect(sanitizeStageName('format:check:all')).toBe('format-check-all');
    expect(sanitizeStageName('rust test')).toBe('rust-test');
  });

  it('formats short and long durations compactly', () => {
    expect(formatDuration(4_500)).toBe('5s');
    expect(formatDuration(70_000)).toBe('1m 10s');
  });

  it('returns the last N non-empty log lines without trailing blank entries', () => {
    expect(tailLines('a\nb\nc\n', 2)).toEqual(['b', 'c']);
  });

  it('summarizes failure logs with matched lines and tail output', () => {
    const summary = summarizeFailureText(
      [
        'Compiling crate',
        'warning: this is noisy',
        'error: build failed',
        'stack trace',
        'more tail',
      ].join('\n'),
    );

    expect(summary).toContain('--- log tail ---');
    expect(summary).toContain('error: build failed');
  });

  it('prunes the oldest log directories first', () => {
    const stalePaths = pruneRunDirectories(
      [
        { path: '/logs/old', mtimeMs: 10 },
        { path: '/logs/newer', mtimeMs: 20 },
        { path: '/logs/newest', mtimeMs: 30 },
      ],
      2,
    );

    expect(stalePaths).toEqual(['/logs/old']);
  });
});
