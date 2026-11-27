import { describe, expect, it } from 'vitest';
import { hookStoppedTurnNotice } from '../workflowStopNotice';

describe('hookStoppedTurnNotice (#3004 G3.1)', () => {
  it('returns the hook stopReason when a hook stopped the turn', () => {
    expect(
      hookStoppedTurnNotice({
        type: 'workflowCompleted',
        sessionId: 's1',
        reason: 'hookStopped',
        stopReason: 'budget exhausted',
      }),
    ).toBe('budget exhausted');
  });

  it('returns null for ordinary natural completions', () => {
    expect(
      hookStoppedTurnNotice({
        type: 'workflowCompleted',
        sessionId: 's1',
        reason: 'natural',
      }),
    ).toBeNull();
  });

  it('returns null when the hook stopped without a reason or with blank text', () => {
    expect(
      hookStoppedTurnNotice({
        type: 'workflowCompleted',
        sessionId: 's1',
        reason: 'hookStopped',
      }),
    ).toBeNull();
    expect(
      hookStoppedTurnNotice({
        type: 'workflowCompleted',
        sessionId: 's1',
        reason: 'hookStopped',
        stopReason: '   ',
      }),
    ).toBeNull();
  });
});
