import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SWRConfig } from 'swr';
import type { ReactNode } from 'react';
import { useAssistantSummaries } from './useAssistantSummaries';
import { listAssistantSummaries } from '@/lib/backend/assistants';
import type { AssistantSummary } from '@/lib/backend/assistants';

const { mockWarn } = vi.hoisted(() => ({
  mockWarn: vi.fn(),
}));

vi.mock('@/lib/backend/assistants', () => ({
  listAssistantSummaries: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    warn: mockWarn,
    error: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const wrapper = ({ children }: { children: ReactNode }) => (
  <SWRConfig
    value={{
      provider: () => new Map(),
      dedupingInterval: 0,
      errorRetryInterval: 1,
    }}
  >
    {children}
  </SWRConfig>
);

describe('useAssistantSummaries', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads assistant summaries and exposes loading state', async () => {
    const summaries: AssistantSummary[] = [
      {
        id: 'assistant-1',
        name: 'Alpha',
        description: 'First assistant',
        deletionProtected: false,
      },
    ];

    vi.mocked(listAssistantSummaries).mockResolvedValueOnce(summaries);

    const { result } = renderHook(() => useAssistantSummaries(), { wrapper });

    expect(result.current.assistants).toEqual([]);
    expect(result.current.loading).toBe(true);
    expect(result.current.error).toBeUndefined();

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.assistants).toEqual(summaries);
    expect(result.current.error).toBeUndefined();
    expect(listAssistantSummaries).toHaveBeenCalledTimes(1);
  });

  it('surfaces fetch errors, logs once, and does not retry automatically', async () => {
    const loadError = new Error('Failed to load assistant summaries');

    vi.mocked(listAssistantSummaries).mockRejectedValueOnce(loadError);

    const { result } = renderHook(() => useAssistantSummaries(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.assistants).toEqual([]);
    expect(result.current.error).toBe(loadError);
    expect(mockWarn).toHaveBeenCalledWith(
      'Failed to load assistant summaries',
      loadError,
    );

    await new Promise((resolve) => setTimeout(resolve, 25));

    expect(listAssistantSummaries).toHaveBeenCalledTimes(1);
    expect(mockWarn).toHaveBeenCalledTimes(1);
  });
});
