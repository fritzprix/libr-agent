import { beforeEach, describe, it, expect, vi } from 'vitest';
import OpenAI from 'openai';
import { AIServiceProvider } from '../types';

const reportListModelsFallback = vi.fn((context: {
  provider: string;
  baseUrl?: string;
  reason: string;
  error: unknown;
}) => ({
  reason: context.reason,
  provider: context.provider,
  baseUrl: context.baseUrl,
  message:
    context.error instanceof Error ? context.error.message : String(context.error),
  usedStaticFallback: true as const,
}));

vi.mock('sonner', () => ({
  toast: { error: vi.fn() },
}));

vi.mock('../list-models-errors', () => ({
  reportListModelsFallback: (context: {
    provider: string;
    baseUrl?: string;
    reason: string;
    error: unknown;
  }) => reportListModelsFallback(context),
}));

vi.mock('../model-capabilities', () => ({
  getContextWindow: vi.fn().mockResolvedValue(128000),
}));

// Mock OpenAI — each construction gets a fresh models.list mock
const listMock = vi.fn();

vi.mock('openai', () => {
  return {
    default: vi.fn().mockImplementation((config) => {
      return {
        _config: config,
        baseURL: config.baseURL,
        models: { list: listMock },
      };
    }),
  };
});

describe('FireworksService', () => {
  beforeEach(() => {
    listMock.mockReset();
    reportListModelsFallback.mockClear();
  });

  it('should construct an instance with the correct API key and baseURL', async () => {
    const { FireworksService } = await import('../fireworks');
    const service = new FireworksService('test-fireworks-key');

    expect(service).toBeInstanceOf(FireworksService);
    expect(OpenAI).toHaveBeenLastCalledWith({
      apiKey: 'test-fireworks-key',
      baseURL: 'https://api.fireworks.ai/inference/v1',
      dangerouslyAllowBrowser: true,
      fetch: expect.any(Function),
    });
  });

  it('should return the correct AIServiceProvider enum', async () => {
    const { FireworksService } = await import('../fireworks');
    const service = new FireworksService('test-fireworks-key');
    expect(service.getProvider()).toBe(AIServiceProvider.Fireworks);
  });

  it('reports listModels fallback under fireworks, not openai', async () => {
    listMock.mockRejectedValue(new Error('Fireworks API down'));

    const { FireworksService } = await import('../fireworks');
    const service = new FireworksService('test-fireworks-key');
    const models = await service.listModels();

    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Fireworks,
        baseUrl: 'https://api.fireworks.ai/inference/v1',
        reason: 'api_error',
      }),
    );
    expect(models).toBeInstanceOf(Array);
    expect(models.length).toBeGreaterThan(0);
  });
});
