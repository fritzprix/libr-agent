import { describe, expect, it } from 'vitest';
import { isTrustedVisualBottom } from '../utils';

describe('isTrustedVisualBottom', () => {
  it('rejects a collapsed-height false bottom while scrollTop is at the list top', () => {
    // Prepend race: distanceFromBottom≈0 but the list is taller than the
    // viewport and scrollTop is still at the top.
    expect(
      isTrustedVisualBottom(0, 0, {
        scrollHeight: 2_000,
        clientHeight: 400,
      }),
    ).toBe(false);
    expect(
      isTrustedVisualBottom(0, 4, {
        scrollHeight: 2_000,
        clientHeight: 400,
      }),
    ).toBe(false);
  });

  it('accepts a real bottom pin when scrollTop is away from the top', () => {
    expect(
      isTrustedVisualBottom(0, 400, {
        scrollHeight: 2_000,
        clientHeight: 400,
      }),
    ).toBe(true);
  });

  it('accepts content that fits entirely in the viewport (top === bottom)', () => {
    expect(
      isTrustedVisualBottom(0, 0, {
        scrollHeight: 300,
        clientHeight: 400,
      }),
    ).toBe(true);
  });

  it('rejects when distance-from-bottom is outside the pin threshold', () => {
    expect(
      isTrustedVisualBottom(20, 400, {
        scrollHeight: 2_000,
        clientHeight: 400,
      }),
    ).toBe(false);
  });
});
