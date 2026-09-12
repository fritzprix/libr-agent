import { describe, expect, it } from 'vitest';
import {
  isPendingApprovalAutoResolvedByMode,
  resolvedApprovalIdsFromModeChange,
} from '../executionModeApprovals';

describe('isPendingApprovalAutoResolvedByMode', () => {
  it('does not auto-resolve any pending approval in normal mode', () => {
    expect(isPendingApprovalAutoResolvedByMode('normal', 'standard')).toBe(
      false,
    );
    expect(isPendingApprovalAutoResolvedByMode('normal', 'hard')).toBe(false);
  });

  it('auto-resolves standard approvals in yolo mode but keeps hard approvals', () => {
    expect(isPendingApprovalAutoResolvedByMode('yolo', 'standard')).toBe(true);
    expect(isPendingApprovalAutoResolvedByMode('yolo', 'hard')).toBe(false);
  });

  it('auto-resolves standard and hard approvals in unsafe mode', () => {
    expect(isPendingApprovalAutoResolvedByMode('unsafe', 'standard')).toBe(
      true,
    );
    expect(isPendingApprovalAutoResolvedByMode('unsafe', 'hard')).toBe(true);
  });
});

describe('resolvedApprovalIdsFromModeChange', () => {
  const fallback = ['local-1', 'local-2'];

  it('uses the local snapshot when the backend returns void', () => {
    expect(resolvedApprovalIdsFromModeChange(undefined, fallback)).toEqual(
      fallback,
    );
    expect(resolvedApprovalIdsFromModeChange(null, fallback)).toEqual(
      fallback,
    );
  });

  it('trusts an empty backend list instead of the local snapshot', () => {
    expect(resolvedApprovalIdsFromModeChange([], fallback)).toEqual([]);
  });

  it('uses backend IDs when the payload is a string array', () => {
    expect(resolvedApprovalIdsFromModeChange(['call-1'], fallback)).toEqual([
      'call-1',
    ]);
  });

  it('falls back when the payload is not a string array', () => {
    expect(resolvedApprovalIdsFromModeChange(['call-1', 2], fallback)).toEqual(
      fallback,
    );
    expect(
      resolvedApprovalIdsFromModeChange({ ids: ['call-1'] }, fallback),
    ).toEqual(fallback);
  });
});
