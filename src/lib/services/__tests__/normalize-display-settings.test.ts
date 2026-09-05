import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SETTING,
  normalizeDisplaySettings,
} from '../settings-service';

describe('normalizeDisplaySettings', () => {
  it('fills missing messageLayout from defaults', () => {
    const normalized = normalizeDisplaySettings({
      metricDisplayMode: 'tooltip',
      prefillDisplayFormat: 'tokensPerSecond',
      showTokenSpeed: false,
      compactMetrics: true,
      toolDetailLevel: 'developer',
      fontFamily: 'Inter',
    });

    expect(normalized.messageLayout).toBe(
      DEFAULT_SETTING.display.messageLayout,
    );
    expect(normalized.metricDisplayMode).toBe('tooltip');
    expect(normalized.fontFamily).toBe('Inter');
  });

  it('preserves a valid messageLayout value', () => {
    const normalized = normalizeDisplaySettings({
      ...DEFAULT_SETTING.display,
      messageLayout: 'bubble',
    });

    expect(normalized.messageLayout).toBe('bubble');
  });

  it('falls back to defaults for invalid blobs', () => {
    expect(normalizeDisplaySettings(null)).toEqual(DEFAULT_SETTING.display);
    expect(normalizeDisplaySettings({ messageLayout: 'wide' })).toEqual(
      DEFAULT_SETTING.display,
    );
  });
});
