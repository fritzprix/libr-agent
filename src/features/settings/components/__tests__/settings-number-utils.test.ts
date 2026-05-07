import { describe, expect, it } from 'vitest';
import {
  formatBytesAsKilobytes,
  parseIntegerInput,
  parseKilobytesInputToBytes,
} from '../settings-number-utils';

describe('settings-number-utils', () => {
  it('parses and clamps integer input', () => {
    expect(parseIntegerInput('12', { fallback: 5, min: 1, max: 10 })).toBe(10);
    expect(parseIntegerInput('-3', { fallback: 5, min: 1, max: 10 })).toBe(1);
  });

  it('falls back for invalid integer input', () => {
    expect(parseIntegerInput('', { fallback: 7, min: 1, max: 10 })).toBe(7);
  });

  it('converts kilobytes input to clamped bytes', () => {
    expect(
      parseKilobytesInputToBytes('32', {
        fallbackKilobytes: 16,
        minKilobytes: 4,
        maxKilobytes: 256,
      }),
    ).toBe(32 * 1024);

    expect(
      parseKilobytesInputToBytes('999', {
        fallbackKilobytes: 16,
        minKilobytes: 4,
        maxKilobytes: 256,
      }),
    ).toBe(256 * 1024);
  });

  it('formats bytes as kilobytes with fallback', () => {
    expect(formatBytesAsKilobytes(undefined, 16 * 1024)).toBe(16);
    expect(formatBytesAsKilobytes(24 * 1024, 16 * 1024)).toBe(24);
  });
});
