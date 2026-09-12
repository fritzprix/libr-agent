import { describe, it, expect } from 'vitest';
import {
  DEFAULT_COMBINATORIAL,
  buildEarlyPool,
  buildEndlessPool,
  fillTemplate,
  parseCombinatorialParts,
  sampleCombinatorial,
} from '../analysisLoaderMessages';

describe('analysisLoaderMessages', () => {
  it('fills combinatorial templates', () => {
    expect(
      fillTemplate('{{action}} {{target}}{{flavor}}', {
        action: 'Sipping',
        target: 'digital coffee',
        flavor: '...',
      }),
    ).toBe('Sipping digital coffee...');
  });

  it('parses combinatorial locale objects and rejects invalid shapes', () => {
    expect(parseCombinatorialParts(DEFAULT_COMBINATORIAL)).toEqual(
      DEFAULT_COMBINATORIAL,
    );
    expect(
      parseCombinatorialParts({ template: 'nope', actions: ['a'] }),
    ).toBeNull();
    expect(parseCombinatorialParts(null)).toBeNull();
  });

  it('samples unique combinatorial messages within capacity', () => {
    const parts = {
      template: '{{action}} {{target}}{{flavor}}',
      actions: ['A', 'B'],
      targets: ['x', 'y'],
      flavors: ['!', '?'],
    };
    const samples = sampleCombinatorial(parts, 8);
    expect(samples).toHaveLength(8);
    expect(new Set(samples).size).toBe(8);
  });

  it('stops early when exclude covers most of a tiny combinatorial space', () => {
    const parts = {
      template: '{{action}} {{target}}{{flavor}}',
      actions: ['A', 'B'],
      targets: ['x'],
      flavors: ['!'],
    };
    // capacity = 2; exclude already holds both possible outputs
    const exclude = new Set(['A x!', 'B x!']);
    const started = performance.now();
    const samples = sampleCombinatorial(parts, 8, exclude);
    const elapsed = performance.now() - started;
    expect(samples).toHaveLength(0);
    expect(elapsed).toBeLessThan(50);
  });

  it('pins the initial message at the front of the early pool', () => {
    const witty = ['one...', 'two...'];
    const pool = buildEarlyPool('start...', witty, DEFAULT_COMBINATORIAL, 10);
    expect(pool[0]).toBe('start...');
    expect(pool.length).toBeGreaterThan(witty.length + 1);
  });

  it('reshuffles endless pools and avoids repeating the last message first', () => {
    const witty = ['alpha', 'beta', 'gamma'];
    const avoid = 'alpha';
    // Probabilistic but constrained: with 3 witty + many combos, first != avoid after rotate
    for (let i = 0; i < 20; i += 1) {
      const pool = buildEndlessPool(witty, DEFAULT_COMBINATORIAL, 12, avoid);
      expect(pool[0]).not.toBe(avoid);
      expect(pool.length).toBeGreaterThan(witty.length);
    }
  });
});
