import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  fetchOpenRouterModels,
  clearMetadataCache,
  findModelMetadata,
  supportsReasoningViaOpenRouter,
  getContextLengthViaOpenRouter,
} from '../openrouter-metadata';

// Mock desktop-fetch module to return a controlled fetch mock
const mockFetch = vi.fn();
vi.mock('../desktop-fetch', () => ({
  createLlmFetch: () => mockFetch,
}));

describe('openrouter-metadata', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearMetadataCache();
  });

  afterEach(() => {
    clearMetadataCache();
  });

  it('single-flights concurrent calls to fetchOpenRouterModels', async () => {
    mockFetch.mockReturnValue(
      new Promise((resolve) =>
        setTimeout(
          () =>
            resolve({
              ok: true,
              json: async () => ({
                data: [
                  {
                    id: 'openai/gpt-4o',
                    name: 'GPT-4o',
                    description: 'OpenAI flagship model',
                    context_length: 128000,
                    architecture: { input_modalities: ['text'], output_modalities: ['text'] },
                    pricing: { prompt: '2.5', completion: '10' },
                    supported_parameters: ['tools'],
                    top_provider: {},
                  },
                ],
              }),
            }),
          50,
        ),
      ),
    );

    // Call fetchOpenRouterModels 10 times simultaneously
    const results = await Promise.all(
      Array.from({ length: 10 }, () => fetchOpenRouterModels()),
    );

    // Network fetch should be called ONLY ONCE
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // All callers should receive the exact same Map instance
    for (const res of results) {
      expect(res).toBe(results[0]);
      expect(res.get('openai/gpt-4o')?.name).toBe('GPT-4o');
    }
  });

  it('returns cached metadata on subsequent calls within TTL', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: async () => ({
        data: [
          {
            id: 'openai/gpt-4o',
            name: 'GPT-4o',
            context_length: 128000,
            pricing: {},
            supported_parameters: [],
          },
        ],
      }),
    });

    const first = await fetchOpenRouterModels();
    expect(mockFetch).toHaveBeenCalledTimes(1);

    const second = await fetchOpenRouterModels();
    expect(mockFetch).toHaveBeenCalledTimes(1); // Cached!
    expect(second).toBe(first);
  });

  it('finds model metadata and resolves context length', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: async () => ({
        data: [
          {
            id: 'anthropic/claude-3.5-sonnet',
            name: 'Claude 3.5 Sonnet',
            context_length: 200000,
            pricing: { internal_reasoning: '15' },
            supported_parameters: ['reasoning'],
          },
        ],
      }),
    });

    const metadata = await findModelMetadata('claude-3.5-sonnet', 'anthropic');
    expect(metadata?.id).toBe('anthropic/claude-3.5-sonnet');

    const contextLength = await getContextLengthViaOpenRouter('claude-3.5-sonnet', 'anthropic');
    expect(contextLength).toBe(200000);

    const reasoning = await supportsReasoningViaOpenRouter('claude-3.5-sonnet', 'anthropic');
    expect(reasoning).toBe(true);
  });
});
