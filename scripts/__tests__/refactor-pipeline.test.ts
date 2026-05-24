import { describe, expect, it } from 'vitest';

import {
  applyStageEnvironment,
  deriveResourceCaps,
  formatDuration,
  getStages,
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

  it('keeps validate as the full validation pipeline', () => {
    expect(getStages('validate').map((stage) => stage.name)).toEqual([
      'sync-builtin-services',
      'format',
      'rust:fmt',
      'lint',
      'format:check:all',
      'test:run',
      'rust:fmt:check',
      'rust:clippy:all',
      'rust:test',
      'rust:check:all',
      'build:nosync',
      'perf:bundle',
      'dead-code',
    ]);
  });

  it('derives adaptive resource caps from host capacity', () => {
    expect(deriveResourceCaps({ cpuCount: 4, totalMemGiB: 8 })).toMatchObject({
      vitestMaxWorkers: 1,
      cargoBuildJobs: 1,
      rustTestThreads: 1,
      uvThreadpoolSize: 1,
    });
    expect(deriveResourceCaps({ cpuCount: 16, totalMemGiB: 32 })).toMatchObject(
      {
        vitestMaxWorkers: 4,
        cargoBuildJobs: 3,
        rustTestThreads: 1,
        uvThreadpoolSize: 2,
      },
    );
  });

  it('applies stage resource caps to the child environment', () => {
    const env = applyStageEnvironment(
      { NODE_OPTIONS: '--trace-warnings' },
      {
        env: {
          nodeHeapMb: 768,
          uvThreadpoolSize: 2,
          cargoBuildJobs: 3,
          rustTestThreads: 1,
          vitestMaxWorkers: 4,
          vitestMinWorkers: 1,
          vitestFileParallelism: false,
        },
      },
    );

    expect(env.NODE_OPTIONS).toContain('--trace-warnings');
    expect(env.NODE_OPTIONS).toContain('--max-old-space-size=768');
    expect(env.UV_THREADPOOL_SIZE).toBe('2');
    expect(env.CARGO_BUILD_JOBS).toBe('3');
    expect(env.RUST_TEST_THREADS).toBe('1');
    expect(env.VITEST_MAX_WORKERS).toBe('4');
    expect(env.VITEST_MIN_WORKERS).toBe('1');
    expect(env.VITEST_FILE_PARALLELISM).toBe('false');
  });
});
