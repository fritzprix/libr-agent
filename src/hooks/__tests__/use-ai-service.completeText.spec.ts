import { renderHook, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { useAIService } from '../use-ai-service';
import { AIServiceFactory } from '@/lib/ai-service';
import type { Message } from '@/models/chat';

// Mock useSettings
vi.mock('../use-settings', () => ({
  useSettings: () => ({
    value: {
      preferredModel: { model: 'gpt-4', provider: 'openai' },
      serviceConfigs: { openai: { apiKey: 'test-key' } },
    },
  }),
}));

type MockAIService = {
  streamChat: ReturnType<typeof vi.fn>;
  cancel: ReturnType<typeof vi.fn>;
};

// Helper: async generator that yields chunks
async function* makeStream(chunks: string[], throwAt?: number) {
  for (let i = 0; i < chunks.length; i++) {
    if (throwAt !== undefined && i === throwAt) {
      throw new Error('stream error');
    }
    yield JSON.stringify({ content: chunks[i] });
    await new Promise((r) => setTimeout(r, 0));
  }
}

// Helper: async generator that throws immediately
async function* makeFailingStream() {
  await new Promise((r) => setTimeout(r, 0)); // Allow async setup
  throw new Error('stream error');
}

describe('useAIService.completeText', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should generate text from a single prompt without history', async () => {
    const fakeService: MockAIService = {
      streamChat: vi
        .fn()
        .mockImplementation(() => makeStream(['Hello', ' ', 'World'])),
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    let finalMessage: Message | undefined;
    await act(async () => {
      finalMessage = await result.current.completeText('Say hello');
    });

    expect(finalMessage).toBeDefined();
    expect(finalMessage?.role).toBe('assistant');
    expect(finalMessage?.content).toBeDefined();
    const contentText = finalMessage?.content
      .map((c) => ('text' in c ? c.text : ''))
      .join('');
    expect(contentText).toBe('Hello World');
  });

  it('should call streamChat with no tools', async () => {
    const streamChatSpy = vi
      .fn()
      .mockImplementation(() => makeStream(['test']));
    const fakeService: MockAIService = {
      streamChat: streamChatSpy,
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    await act(async () => {
      await result.current.completeText('Test prompt');
    });

    expect(streamChatSpy).toHaveBeenCalledTimes(1);
    const callArgs = streamChatSpy.mock.calls[0];
    const options = callArgs[1];

    // Verify no tools are passed
    expect(options.availableTools).toEqual([]);
    expect(options.forceToolUse).toBe(false);
  });

  it('should call onProgress callback during streaming', async () => {
    const fakeService: MockAIService = {
      streamChat: vi
        .fn()
        .mockImplementation(() => makeStream(['part1', 'part2'])),
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    const progressCalls: Array<{ partial: string; isFinal?: boolean }> = [];
    const onProgress = (partial: string, isFinal?: boolean) => {
      progressCalls.push({ partial, isFinal });
    };

    await act(async () => {
      await result.current.completeText('Test', { onProgress });
    });

    expect(progressCalls.length).toBeGreaterThan(0);
    // Last call should be final
    const lastCall = progressCalls[progressCalls.length - 1];
    expect(lastCall.isFinal).toBe(true);
  });

  it('should return error message on stream failure', async () => {
    const fakeService: MockAIService = {
      streamChat: vi.fn().mockImplementation(() => makeFailingStream()), // Use failing stream
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    let errorMessage: Message | undefined;
    await act(async () => {
      errorMessage = await result.current.completeText('Fail prompt');
    });

    expect(errorMessage).toBeDefined();
    expect(errorMessage?.role).toBe('assistant');
    // Error message should contain error indication
    const contentText = errorMessage?.content
      .map((c) => ('text' in c ? c.text : ''))
      .join('')
      .toLowerCase();
    expect(contentText).toMatch(/error|issue|problem|apologize/i);
  });

  it('should use custom model and systemPrompt from options', async () => {
    const streamChatSpy = vi
      .fn()
      .mockImplementation(() => makeStream(['response']));
    const fakeService: MockAIService = {
      streamChat: streamChatSpy,
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    const customSystemPrompt = 'You are a test assistant';
    const customModel = 'gpt-4';

    await act(async () => {
      await result.current.completeText('Test', {
        model: customModel,
        systemPrompt: customSystemPrompt,
      });
    });

    expect(streamChatSpy).toHaveBeenCalledTimes(1);
    const options = streamChatSpy.mock.calls[0][1];

    expect(options.modelName).toBe(customModel);
    expect(options.systemPrompt).toBe(customSystemPrompt);
  });

  it('should handle empty response gracefully', async () => {
    const fakeService: MockAIService = {
      streamChat: vi.fn().mockImplementation(() => makeStream([])),
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    let finalMessage: Message | undefined;
    await act(async () => {
      finalMessage = await result.current.completeText('Empty test');
    });

    expect(finalMessage).toBeDefined();
    expect(finalMessage?.role).toBe('assistant');
    const contentText = finalMessage?.content
      .map((c) => ('text' in c ? c.text : ''))
      .join('');
    expect(contentText).toBe('No response generated.');
  });

  it('should ignore tool_calls in stream chunks', async () => {
    async function* streamWithTools() {
      yield JSON.stringify({
        content: 'text',
        tool_calls: [{ id: 'test', function: { name: 'test' } }],
      });
      yield JSON.stringify({ content: ' more text' });
    }

    const fakeService: MockAIService = {
      streamChat: vi.fn().mockImplementation(() => streamWithTools()),
      cancel: vi.fn(),
    };
    vi.spyOn(AIServiceFactory, 'getService').mockReturnValue(
      fakeService as unknown as ReturnType<typeof AIServiceFactory.getService>,
    );

    const { result } = renderHook(() => useAIService());

    let finalMessage: Message | undefined;
    await act(async () => {
      finalMessage = await result.current.completeText('Test with tools');
    });

    expect(finalMessage).toBeDefined();
    // Should only contain text content, no tool_calls in final message
    expect(finalMessage?.tool_calls).toBeUndefined();
    const contentText = finalMessage?.content
      .map((c) => ('text' in c ? c.text : ''))
      .join('');
    expect(contentText).toBe('text more text');
  });
});
