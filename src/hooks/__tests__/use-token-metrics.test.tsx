import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useTokenMetrics } from '../use-token-metrics';
import { useStreamingMessage } from '@/context/LLMServiceContext';
import type { Message } from '@/models/chat';

vi.mock('@/context/LLMServiceContext', () => ({
  useStreamingMessage: vi.fn(),
}));

describe('useTokenMetrics', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns null metrics when there is no streaming message', () => {
    vi.mocked(useStreamingMessage).mockReturnValue(undefined);

    const { result } = renderHook(() => useTokenMetrics('session-1'));

    expect(result.current.metrics).toBeNull();
  });

  it('returns null metrics when streaming message has no usage data', () => {
    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Thinking...',
    } as unknown as Partial<Message>);

    const { result } = renderHook(() => useTokenMetrics('session-1'));

    expect(result.current.metrics).toBeNull();
  });

  it('preserves return object and metrics reference when streaming message content updates without usage changes', () => {
    const usage = {
      promptTokens: 100,
      completionTokens: 20,
      totalTokens: 120,
    };

    // First render with initial content
    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello',
      usage,
    } as unknown as Partial<Message>);

    const { result, rerender } = renderHook(() => useTokenMetrics('session-1'));

    const initialResult = result.current;
    expect(initialResult.metrics).toEqual(usage);

    // Second render with new token appended to content, but same usage reference
    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello world',
      usage,
    } as unknown as Partial<Message>);

    rerender();

    expect(result.current).toBe(initialResult);
    expect(result.current.metrics).toBe(initialResult.metrics);

    // Third render with newly allocated usage object having identical values (structural equality)
    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello world!',
      usage: {
        promptTokens: 100,
        completionTokens: 20,
        totalTokens: 120,
      },
    } as unknown as Partial<Message>);

    rerender();

    expect(result.current).toBe(initialResult);
    expect(result.current.metrics).toBe(initialResult.metrics);
  });

  it('updates metrics and return reference when token metrics change', () => {
    const initialUsage = {
      promptTokens: 100,
      completionTokens: 20,
      totalTokens: 120,
    };

    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello',
      usage: initialUsage,
    } as unknown as Partial<Message>);

    const { result, rerender } = renderHook(() => useTokenMetrics('session-1'));

    const initialResult = result.current;
    expect(initialResult.metrics).toBe(initialUsage);

    const updatedUsage = {
      promptTokens: 100,
      completionTokens: 25,
      totalTokens: 125,
    };

    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello world',
      usage: updatedUsage,
    } as unknown as Partial<Message>);

    rerender();

    expect(result.current).not.toBe(initialResult);
    expect(result.current.metrics).toBe(updatedUsage);
  });

  it('updates metrics when token counts are identical but details change (e.g. timeToFirstToken)', () => {
    const initialUsage = {
      promptTokens: 100,
      completionTokens: 20,
      totalTokens: 120,
      details: {
        timeToFirstToken: 0,
      },
    };

    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello',
      usage: initialUsage,
    } as unknown as Partial<Message>);

    const { result, rerender } = renderHook(() => useTokenMetrics('session-1'));

    const initialResult = result.current;
    expect(initialResult.metrics).toBe(initialUsage);

    // Identical counts, but timeToFirstToken updated from 0 to 120
    const updatedUsage = {
      promptTokens: 100,
      completionTokens: 20,
      totalTokens: 120,
      details: {
        timeToFirstToken: 120,
      },
    };

    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello',
      usage: updatedUsage,
    } as unknown as Partial<Message>);

    rerender();

    expect(result.current).not.toBe(initialResult);
    expect(result.current.metrics).toBe(updatedUsage);
    expect(result.current.metrics?.details?.timeToFirstToken).toBe(120);

    // And verify structural equality when details have the same keys and values in a new object
    const sameDetailsNewObject = {
      promptTokens: 100,
      completionTokens: 20,
      totalTokens: 120,
      details: {
        timeToFirstToken: 120,
      },
    };

    const currentResult = result.current;
    vi.mocked(useStreamingMessage).mockReturnValue({
      id: 'msg-1',
      content: 'Hello',
      usage: sameDetailsNewObject,
    } as unknown as Partial<Message>);

    rerender();

    expect(result.current).toBe(currentResult);
    expect(result.current.metrics).toBe(currentResult.metrics);
  });
});

