import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  estimateContextWindow,
  registerAIServiceFactory,
  supportsTools,
} from '../model-capabilities';
import { AIServiceProvider } from '../types';
import { OpenAIService } from '../openai';

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('model-capabilities', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('delegates capability checks without creating runtime service instances', () => {
    const getService = vi.fn();
    const getCapabilityDelegate = vi.fn(() => ({
      supportsTools: vi.fn().mockReturnValue(true),
      estimateContextWindow: vi.fn().mockReturnValue(128000),
    }));
    const factory = {
      getCapabilityDelegate,
      getService,
    };

    registerAIServiceFactory(factory);

    expect(supportsTools('gpt-4o', AIServiceProvider.OpenAI)).toBe(true);
    expect(
      estimateContextWindow('gpt-4o', AIServiceProvider.OpenAI),
    ).toBe(128000);
    expect(getCapabilityDelegate).toHaveBeenCalledWith(AIServiceProvider.OpenAI);
    expect(getService).not.toHaveBeenCalled();
  });

  it('matches only known OpenAI o-series prefixes for tool support', () => {
    expect(OpenAIService.supportsToolsForModel('o3-mini')).toBe(true);
    expect(OpenAIService.supportsToolsForModel('o4-mini')).toBe(true);
    expect(OpenAIService.supportsToolsForModel('orca-mini')).toBe(false);
    expect(OpenAIService.supportsToolsForModel('gpt-4o')).toBe(true);
  });
});
