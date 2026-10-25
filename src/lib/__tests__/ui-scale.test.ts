import { describe, expect, it } from 'vitest';
import {
  DEFAULT_UI_SCALE,
  normalizeUiScale,
  resolveUiScaleShortcut,
  stepUiScale,
} from '../ui-scale';

const key = (k: string, mods: Partial<KeyboardEvent> = {}) => ({
  key: k,
  metaKey: false,
  ctrlKey: true,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe('ui-scale', () => {
  it('snaps unknown values to the default', () => {
    expect(normalizeUiScale(1.25)).toBe(1.25);
    expect(normalizeUiScale(1.3)).toBe(DEFAULT_UI_SCALE);
    expect(normalizeUiScale('1.5')).toBe(DEFAULT_UI_SCALE);
    expect(normalizeUiScale(undefined)).toBe(DEFAULT_UI_SCALE);
  });

  it('steps through the fixed scales and clamps at the ends', () => {
    expect(stepUiScale(1, 1)).toBe(1.1);
    expect(stepUiScale(1.1, 1)).toBe(1.25);
    expect(stepUiScale(1.5, 1)).toBe(1.5);
    expect(stepUiScale(1, -1)).toBe(0.9);
    expect(stepUiScale(0.9, -1)).toBe(0.9);
  });

  it('maps Ctrl/Cmd +, =, -, 0 and ignores other keys', () => {
    expect(resolveUiScaleShortcut(key('+'))).toBe('up');
    expect(resolveUiScaleShortcut(key('=', { ctrlKey: false, metaKey: true }))).toBe('up');
    expect(resolveUiScaleShortcut(key('-'))).toBe('down');
    expect(resolveUiScaleShortcut(key('0'))).toBe('reset');
    expect(resolveUiScaleShortcut(key('+', { ctrlKey: false }))).toBeNull();
    expect(resolveUiScaleShortcut(key('+', { altKey: true }))).toBeNull();
    expect(resolveUiScaleShortcut(key('a'))).toBeNull();
  });
});
