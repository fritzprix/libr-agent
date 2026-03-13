import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAssistantSkills } from '../hooks/useAssistantSkills';
import { useEditor } from '@/context/EditorContext';
import { 
  getAggregatedSkills, 
  copyGlobalToAssistant, 
  deleteAssistantSkill, 
  resetAssistantSkills 
} from '@/lib/backend/skills';
import { SkillMetadata } from '@/types/skills';
import { toast } from 'sonner';
import { SWRConfig } from 'swr';
import React from 'react';

vi.mock('@/context/EditorContext', () => ({
  useEditor: vi.fn(),
}));

const mockT = vi.fn((key: string) => key);
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: mockT }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/lib/backend/skills', () => ({
  getAggregatedSkills: vi.fn(),
  copyGlobalToAssistant: vi.fn(),
  deleteAssistantSkill: vi.fn(),
  resetAssistantSkills: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('useAssistantSkills', () => {
  const mockDraft = { id: 'assistant-1' };
  const mockUpdate = vi.fn();

  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      {children}
    </SWRConfig>
  );

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useEditor).mockReturnValue({
      draft: mockDraft,
      update: mockUpdate,
    } as unknown as ReturnType<typeof useEditor>);
    vi.mocked(getAggregatedSkills).mockResolvedValue([]);
  });

  const createMockSkill = (name: string, source: 'global' | 'assistant' = 'global'): SkillMetadata => ({
    name,
    description: `Description for ${name}`,
    path: `/path/to/${name}`,
    source,
  });

  it('fetches skills on mount', async () => {
    const mockSkills = [createMockSkill('skill-1')];
    vi.mocked(getAggregatedSkills).mockResolvedValueOnce(mockSkills);

    const { result } = renderHook(() => useAssistantSkills(), { wrapper });
    
    expect(result.current.isLoading).toBe(true);
    await waitFor(() => {
      expect(result.current.skills).toEqual(mockSkills);
      expect(result.current.isLoading).toBe(false);
    });
  });

  it('prevents stale updates when switching assistants', async () => {
    let resolveFirst!: (val: SkillMetadata[]) => void;
    vi.mocked(getAggregatedSkills).mockReturnValueOnce(
      new Promise((res) => { resolveFirst = res; })
    );

    const { result, rerender } = renderHook(() => useAssistantSkills(), { wrapper });

    // Set up second assistant's skills
    vi.mocked(getAggregatedSkills).mockResolvedValueOnce([createMockSkill('fresh-skill')]);

    // Switch assistant
    vi.mocked(useEditor).mockReturnValue({
      draft: { id: 'assistant-2' },
      update: mockUpdate,
    } as unknown as ReturnType<typeof useEditor>);
    
    rerender();

    // Resolve first assistant's skills late
    await act(async () => {
      resolveFirst([createMockSkill('stale-skill')]);
    });
    
    await waitFor(() => {
      expect(result.current.skills).toEqual([createMockSkill('fresh-skill')]);
    });
  });

  it('handles skill override', async () => {
    vi.mocked(copyGlobalToAssistant).mockResolvedValueOnce('assistant-1/skill-1');
    vi.mocked(getAggregatedSkills).mockResolvedValue([createMockSkill('skill-1', 'assistant')]);

    const { result } = renderHook(() => useAssistantSkills(), { wrapper });
    
    // Initial fetch
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleOverride('skill-1');
    });

    expect(copyGlobalToAssistant).toHaveBeenCalledWith('assistant-1', 'skill-1');
    expect(toast.success).toHaveBeenCalledWith('skills.overrideSuccess');
    expect(getAggregatedSkills).toHaveBeenCalledTimes(2);
  });

  it('handles skill revert', async () => {
    vi.mocked(deleteAssistantSkill).mockResolvedValueOnce('skill-1');
    vi.mocked(getAggregatedSkills).mockResolvedValue([createMockSkill('skill-1', 'global')]);

    const { result } = renderHook(() => useAssistantSkills(), { wrapper });
    
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleRevert('skill-1');
    });

    expect(deleteAssistantSkill).toHaveBeenCalledWith('assistant-1', 'skill-1');
    expect(toast.success).toHaveBeenCalledWith('skills.revertSuccess');
  });

  it('handles reset', async () => {
    vi.mocked(resetAssistantSkills).mockResolvedValueOnce('assistant-1');
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useAssistantSkills(), { wrapper });
    
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.confirmReset(onSuccess);
    });

    expect(resetAssistantSkills).toHaveBeenCalledWith('assistant-1');
    expect(toast.success).toHaveBeenCalledWith('skills.resetSuccess');
    expect(onSuccess).toHaveBeenCalled();
  });
});
