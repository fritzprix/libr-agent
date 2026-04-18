import { describe, expect, it } from 'vitest';
import {
  evaluateBundleBudget,
  formatBytes,
  summarizeBundleAssets,
  type BundleBudget,
} from '../lib/bundle-budget';

describe('bundle-budget', () => {
  it('summarizes js and css totals and largest assets', () => {
    const summary = summarizeBundleAssets([
      { name: 'index.js', size: 1800 },
      { name: 'chunk.js', size: 900 },
      { name: 'index.css', size: 400 },
      { name: 'font.woff2', size: 3000 },
    ]);

    expect(summary.totalBytes).toBe(6100);
    expect(summary.totalJsBytes).toBe(2700);
    expect(summary.totalCssBytes).toBe(400);
    expect(summary.largestJsAsset).toMatchObject({
      name: 'index.js',
      size: 1800,
    });
    expect(summary.largestCssAsset).toMatchObject({
      name: 'index.css',
      size: 400,
    });
  });

  it('reports only metrics that exceed the configured budget', () => {
    const budget: BundleBudget = {
      totalJsBytes: 2500,
      totalCssBytes: 350,
      largestJsBytes: 1700,
      largestCssBytes: 500,
    };

    const summary = summarizeBundleAssets([
      { name: 'index.js', size: 1800 },
      { name: 'chunk.js', size: 900 },
      { name: 'index.css', size: 400 },
    ]);

    expect(evaluateBundleBudget(summary, budget)).toEqual([
      { metric: 'totalJsBytes', actual: 2700, limit: 2500 },
      { metric: 'totalCssBytes', actual: 400, limit: 350 },
      { metric: 'largestJsBytes', actual: 1800, limit: 1700 },
    ]);
  });

  it('formats byte counts for human-readable reports', () => {
    expect(formatBytes(900)).toBe('900 B');
    expect(formatBytes(2048)).toBe('2.0 KiB');
    expect(formatBytes(2_621_440)).toBe('2.50 MiB');
  });
});
