import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  isNpxRuntimeReady,
  probeRuntimeBinaries,
} from './runtimeProbe';

const safeInvoke = vi.fn();

vi.mock('./core', () => ({
  safeInvoke: (...args: unknown[]) => safeInvoke(...args),
}));

describe('runtimeProbe', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes probe_runtime_binaries', async () => {
    const result = {
      node: true,
      npx: true,
      python: true,
      uv: false,
    };
    safeInvoke.mockResolvedValue(result);

    await expect(probeRuntimeBinaries()).resolves.toEqual(result);
    expect(safeInvoke).toHaveBeenCalledWith('probe_runtime_binaries');
  });

  it('treats npx presence as readiness for npx MCP servers', () => {
    expect(
      isNpxRuntimeReady({
        node: true,
        npx: true,
        python: false,
        uv: false,
      }),
    ).toBe(true);
    expect(
      isNpxRuntimeReady({
        node: true,
        npx: false,
        python: true,
        uv: true,
      }),
    ).toBe(false);
    expect(isNpxRuntimeReady(null)).toBe(false);
  });
});
