import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SWRConfig } from 'swr';
import { useScopedSkills } from './useScopedSkills';
import { getAggregatedSkills } from '@/lib/backend/skills';
import type { SkillMetadata } from '@/types/skills';

let sessionUpdateCallback: (() => void) | undefined;

vi.mock('@/lib/backend/skills', () => ({
  getAggregatedSkills: vi.fn(),
}));

vi.mock('@/context/GlobalEventContext', () => ({
  useBackendResource: (
    _resourceType: 'session',
    callback: () => void,
  ) => {
    sessionUpdateCallback = callback;
  },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const alphaSkill: SkillMetadata = {
  name: 'alpha',
  description: 'alpha description',
  path: 'C:\\skills\\alpha\\SKILL.md',
  source: 'workspace',
};

const betaSkill: SkillMetadata = {
  name: 'beta',
  description: 'beta description',
  path: 'C:\\skills\\beta\\SKILL.md',
  source: 'workspace',
};

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
    {children}
  </SWRConfig>
);

describe('useScopedSkills', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sessionUpdateCallback = undefined;
  });

  it('loads scoped skills with the provided session scope', async () => {
    vi.mocked(getAggregatedSkills).mockResolvedValueOnce([alphaSkill]);

    const { result } = renderHook(
      () =>
        useScopedSkills({
          assistantId: 'assistant-1',
          sessionId: 'session-1',
        }),
      { wrapper },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.skills).toEqual([alphaSkill]);
    expect(getAggregatedSkills).toHaveBeenCalledWith('assistant-1', {
      sessionId: 'session-1',
      workspacePath: undefined,
    });
  });

  it('revalidates when the current session scope is updated', async () => {
    vi.mocked(getAggregatedSkills)
      .mockResolvedValueOnce([alphaSkill])
      .mockResolvedValueOnce([betaSkill]);

    const { result } = renderHook(
      () =>
        useScopedSkills({
          assistantId: 'assistant-1',
          sessionId: 'session-1',
        }),
      { wrapper },
    );

    await waitFor(() => expect(result.current.skills).toEqual([alphaSkill]));

    await act(async () => {
      sessionUpdateCallback?.();
    });

    await waitFor(() => expect(result.current.skills).toEqual([betaSkill]));
    expect(getAggregatedSkills).toHaveBeenCalledTimes(2);
  });
});
