import { createRequire } from 'node:module';
import { describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const { planWorkspaceModes } = require('../lib/bench-workspace-mode.cjs');

describe('planWorkspaceModes', () => {
  it('defaults an isolated run to host', () => {
    const plan = planWorkspaceModes(['--preset', 'path'], true, undefined);
    expect(plan).toEqual({
      argv: ['--preset', 'path'],
      modes: ['host'],
      immediate: null,
      error: null,
    });
  });

  it('defaults a non-isolated run to managed and exports it right away', () => {
    const plan = planWorkspaceModes(['--preset', 'hello'], false, undefined);
    expect(plan.modes).toEqual(['managed']);
    expect(plan.immediate).toBe('managed');
    expect(plan.error).toBeNull();
  });

  it('takes the flag out of argv and honors it', () => {
    const plan = planWorkspaceModes(
      ['--workspace-mode', 'managed', '--preset', 'path'],
      true,
      undefined,
    );
    expect(plan.argv).toEqual(['--preset', 'path']);
    expect(plan.modes).toEqual(['managed']);
  });

  it('runs every mode for all', () => {
    expect(
      planWorkspaceModes(['--workspace-mode', 'all'], true, undefined).modes,
    ).toEqual(['managed', 'host']);
  });

  it('rejects host and all without --isolated', () => {
    for (const mode of ['host', 'all']) {
      expect(
        planWorkspaceModes(['--workspace-mode', mode], false, undefined).error,
      ).toMatch(/requires --isolated/);
    }
  });

  it('rejects attach, unknown values, and a missing value', () => {
    for (const argv of [
      ['--workspace-mode', 'attach'],
      ['--workspace-mode', 'docker'],
      ['--workspace-mode'],
    ]) {
      expect(planWorkspaceModes(argv, true, undefined).error).toMatch(
        /must be one of/,
      );
    }
  });

  it('respects a preset env mode instead of overriding it', () => {
    expect(planWorkspaceModes([], true, 'managed').modes).toEqual(['managed']);
    expect(planWorkspaceModes([], false, 'host').error).toMatch(
      /requires --isolated/,
    );
    expect(planWorkspaceModes([], true, 'attach').error).toMatch(
      /must be one of/,
    );
  });
});
