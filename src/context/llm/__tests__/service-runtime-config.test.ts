import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTING } from '@/lib/services/settings-service';
import { buildServiceRuntimeConfig } from '../service-runtime-config';

describe('buildServiceRuntimeConfig', () => {
  it('omits temperature when override is disabled', () => {
    const config = buildServiceRuntimeConfig({
      ...DEFAULT_SETTING,
      temperatureOverrideEnabled: false,
      temperature: 0.9,
    });

    expect(config.temperature).toBeUndefined();
    expect(config.maxRetries).toBe(DEFAULT_SETTING.advanced.maxRetries);
    expect(config.retryDelay).toBe(DEFAULT_SETTING.advanced.retryDelay);
  });

  it('includes temperature when override is enabled', () => {
    const config = buildServiceRuntimeConfig({
      ...DEFAULT_SETTING,
      temperatureOverrideEnabled: true,
      temperature: 0.3,
    });

    expect(config.temperature).toBe(0.3);
  });

  it('lets explicit overrides win over settings temperature', () => {
    const config = buildServiceRuntimeConfig(
      {
        ...DEFAULT_SETTING,
        temperatureOverrideEnabled: true,
        temperature: 0.3,
      },
      {},
      { temperature: 1.2 },
    );

    expect(config.temperature).toBe(1.2);
  });

  it('includes thinkingBudget from advanced settings', () => {
    const config = buildServiceRuntimeConfig({
      ...DEFAULT_SETTING,
      advanced: {
        ...DEFAULT_SETTING.advanced,
        thinkingBudget: -1,
      },
    });

    expect(config.thinkingBudget).toBe(-1);
  });

  it('includes thinkingBudget when disabled (0)', () => {
    const config = buildServiceRuntimeConfig({
      ...DEFAULT_SETTING,
      advanced: {
        ...DEFAULT_SETTING.advanced,
        thinkingBudget: 0,
      },
    });

    expect(config.thinkingBudget).toBe(0);
  });
});
