import { describe, expect, it } from 'vitest';
import { buildServiceRuntimeConfig } from '../service-runtime-config';
import { TEST_ADVANCED_SETTINGS, TEST_SETTINGS } from '@/test/fixtures/settings';

describe('buildServiceRuntimeConfig', () => {
  it('includes retry settings from advanced settings', () => {
    const settings = {
      ...TEST_SETTINGS,
      advanced: {
        ...TEST_ADVANCED_SETTINGS,
        maxRetries: 3,
        retryDelay: 1000,
      },
    };
    const config = buildServiceRuntimeConfig(settings);
    expect(config.maxRetries).toBe(3);
    expect(config.retryDelay).toBe(1000);
  });

  it('includes temperature when override is enabled', () => {
    const settings = {
      ...TEST_SETTINGS,
      temperatureOverrideEnabled: true,
      temperature: 0.5,
    };
    const config = buildServiceRuntimeConfig(settings);
    expect(config.temperature).toBe(0.5);
  });

  it('omits temperature when override is disabled', () => {
    const settings = {
      ...TEST_SETTINGS,
      temperatureOverrideEnabled: false,
      temperature: 0.5,
    };
    const config = buildServiceRuntimeConfig(settings);
    expect(config.temperature).toBeUndefined();
  });

  it('includes thinkingEffort from advanced settings', () => {
    const settings = {
      ...TEST_SETTINGS,
      advanced: {
        ...TEST_ADVANCED_SETTINGS,
        thinkingEffort: 'auto' as const,
      },
    };
    const config = buildServiceRuntimeConfig(settings);
    expect(config.thinkingEffort).toBe('auto');
  });

  it('includes thinkingEffort when disabled (off)', () => {
    const settings = {
      ...TEST_SETTINGS,
      advanced: {
        ...TEST_ADVANCED_SETTINGS,
        thinkingEffort: 'off' as const,
      },
    };
    const config = buildServiceRuntimeConfig(settings);
    expect(config.thinkingEffort).toBe('off');
  });
});
