import { describe, expect, it } from 'vitest';

import type { SessionRuntimeState } from '@/models/agent-ipc';
import {
  pickRuntimeState,
  shouldApplyRuntimeState,
} from '../runtimeStateApply';

function makeState(
  sequence: number,
  ready: boolean,
): SessionRuntimeState {
  return {
    sequence,
    phase: ready ? 'ready' : 'hydrating',
    proxy: {
      exists: ready,
      mode: ready ? 'builtin_only' : 'none',
      ready,
    },
    initialization: {
      result: ready ? 'success' : 'pending',
    },
    servers: [],
  };
}

describe('runtimeStateApply', () => {
  it('applies equal or newer sequences', () => {
    expect(shouldApplyRuntimeState(makeState(2, true), makeState(2, false))).toBe(
      true,
    );
    expect(shouldApplyRuntimeState(makeState(2, true), makeState(3, true))).toBe(
      true,
    );
  });

  it('rejects older sequences so live Ready wins over stale Hydrating open()', () => {
    const liveReady = makeState(5, true);
    const staleOpen = makeState(1, false);

    expect(shouldApplyRuntimeState(liveReady, staleOpen)).toBe(false);
    expect(pickRuntimeState(liveReady, staleOpen)).toBe(liveReady);
  });
});
