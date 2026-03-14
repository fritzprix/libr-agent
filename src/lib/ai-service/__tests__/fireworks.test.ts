import { describe, it, expect, vi } from 'vitest';
import { FireworksService } from '../fireworks';
import { AIServiceProvider } from '../types';
import OpenAI from 'openai';

// Mock OpenAI
vi.mock('openai', () => {
  return {
    default: vi.fn().mockImplementation((config) => {
      return { _config: config };
    }),
  };
});

describe('FireworksService', () => {
  it('should construct an instance with the correct API key and baseURL', () => {
    const service = new FireworksService('test-fireworks-key');

    // Check that it's an instance of the class
    expect(service).toBeInstanceOf(FireworksService);

    // Check constructor behavior
    expect(OpenAI).toHaveBeenCalledWith({
      apiKey: 'test-fireworks-key',
      baseURL: 'https://api.fireworks.ai/inference/v1',
      dangerouslyAllowBrowser: true,
    });
  });

  it('should return the correct AIServiceProvider enum', () => {
    const service = new FireworksService('test-fireworks-key');
    expect(service.getProvider()).toBe(AIServiceProvider.Fireworks);
  });
});
