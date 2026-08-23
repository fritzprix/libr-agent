import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SETTING,
  normalizeExperimentalSettings,
} from '../settings-service';

describe('normalizeExperimentalSettings', () => {
  it('returns defaults for empty / invalid blobs', () => {
    expect(normalizeExperimentalSettings(undefined)).toEqual({
      experimental: DEFAULT_SETTING.experimental,
      didMigrate: false,
    });
    expect(normalizeExperimentalSettings(null)).toEqual({
      experimental: DEFAULT_SETTING.experimental,
      didMigrate: false,
    });
  });

  it('keeps canonical policy without migration', () => {
    const { experimental, didMigrate } = normalizeExperimentalSettings({
      inlineAudioAttachment: false,
      toolLoopRecoveryPolicy: 'legacyGuidance',
      toolLoopMaxResampleRetries: 4,
    });

    expect(didMigrate).toBe(false);
    expect(experimental).toEqual({
      inlineAudioAttachment: false,
      toolLoopRecoveryPolicy: 'legacyGuidance',
      toolLoopMaxResampleRetries: 4,
    });
  });

  it('migrates toolLoopLegacyGuidanceEnabled true → legacyGuidance', () => {
    const { experimental, didMigrate } = normalizeExperimentalSettings({
      toolLoopLegacyGuidanceEnabled: true,
      toolLoopMaxResampleRetries: 1,
    });

    expect(didMigrate).toBe(true);
    expect(experimental.toolLoopRecoveryPolicy).toBe('legacyGuidance');
    expect(experimental.toolLoopMaxResampleRetries).toBe(1);
    expect(
      Object.prototype.hasOwnProperty.call(
        experimental,
        'toolLoopLegacyGuidanceEnabled',
      ),
    ).toBe(false);
  });

  it('migrates toolLoopLegacyGuidanceEnabled false → resampleThenBreak', () => {
    const { experimental, didMigrate } = normalizeExperimentalSettings({
      inlineAudioAttachment: true,
      toolLoopLegacyGuidanceEnabled: false,
    });

    expect(didMigrate).toBe(true);
    expect(experimental.toolLoopRecoveryPolicy).toBe('resampleThenBreak');
  });

  it('prefers canonical policy when both keys are present', () => {
    const { experimental, didMigrate } = normalizeExperimentalSettings({
      toolLoopRecoveryPolicy: 'resampleThenBreak',
      toolLoopLegacyGuidanceEnabled: true,
    });

    expect(didMigrate).toBe(true);
    expect(experimental.toolLoopRecoveryPolicy).toBe('resampleThenBreak');
  });

  it('clamps max resample retries', () => {
    expect(
      normalizeExperimentalSettings({ toolLoopMaxResampleRetries: 99 })
        .experimental.toolLoopMaxResampleRetries,
    ).toBe(20);
    expect(
      normalizeExperimentalSettings({ toolLoopMaxResampleRetries: -3 })
        .experimental.toolLoopMaxResampleRetries,
    ).toBe(0);
  });
});
