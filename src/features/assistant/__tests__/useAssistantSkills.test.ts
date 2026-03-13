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
import { toast } from 'sonner';

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

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useEditor).mockReturnValue({
      draft: mockDraft,
      update: mockUpdate,
    } as any);
    vi.mocked(getAggregatedSkills).mockResolvedValue([]);
  });

  it('fetches skills on mount', async () => {
    const mockSkills = [{ name: 'skill-1', isAssistantSpecific: false }];
    vi.mocked(getAggregatedSkills).mockResolvedValueOnce(mockSkills);

    const { result } = renderHook(() => useAssistantSkills());
    
    expect(result.current.isLoading).toBe(true);
    await waitFor(() => {
      expect(result.current.skills).toEqual(mockSkills);
      expect(result.current.isLoading).toBe(false);
    });
  });

  it('prevents stale updates when switching assistants', async () => {
    let resolveFirst!: (val: any) => void;
    vi.mocked(getAggregatedSkills).mockReturnValueOnce(
      new Promise((res) => { resolveFirst = res; })
    );

    const { result, rerender } = renderHook(() => useAssistantSkills());

    // Set up second assistant's skills
    vi.mocked(getAggregatedSkills).mockResolvedValueOnce([{ name: 'fresh-skill' }]);

    // Switch assistant
    vi.mocked(useEditor).mockReturnValue({
      draft: { id: 'assistant-2' },
      update: mockUpdate,
    } as any);
    
    rerender();

    // Resolve first assistant's skills late
    resolveFirst([{ name: 'stale-skill' }]);
    
    await waitFor(() => {
      expect(result.current.skills).toEqual([{ name: 'fresh-skill' }]);
    });
  });

  it('handles skill override', async () => {
    vi.mocked(copyGlobalToAssistant).mockResolvedValueOnce(undefined);
    vi.mocked(getAggregatedSkills).mockResolvedValue([{ name: 'skill-1', isAssistantSpecific: true }]);

    const { result } = renderHook(() => useAssistantSkills());
    
    await act(async () => {
      await result.current.handleOverride('skill-1');
    });

    expect(copyGlobalToAssistant).toHaveBeenCalledWith('assistant-1', 'skill-1');
    expect(toast.success).toHaveBeenCalledWith('skills.overrideSuccess');
    expect(getAggregatedSkills).toHaveBeenCalledTimes(2); // Initial + after override
  });

  it('handles skill revert', async () => {
    vi.mocked(deleteAssistantSkill).mockResolvedValueOnce(undefined);
    vi.mocked(getAggregatedSkills).mockResolvedValue([{ name: 'skill-1', isAssistantSpecific: false }]);

    const { result } = renderHook(() => useAssistantSkills());
    
    await act(async () => {
      await result.current.handleRevert('skill-1');
    });

    expect(deleteAssistantSkill).toHaveBeenCalledWith('assistant-1', 'skill-1');
    expect(toast.success).toHaveBeenCalledWith('skills.revertSuccess');
  });

  it('handles reset', async () => {
    vi.mocked(resetAssistantSkills).mockResolvedValueOnce(undefined);
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useAssistantSkills());
    
    await act(async () => {
      await result.current.confirmReset(onSuccess);
    });

    expect(resetAssistantSkills).toHaveBeenCalledWith('assistant-1');
    expect(toast.success).toHaveBeenCalledWith('skills.resetSuccess');
    expect(onSuccess).toHaveBeenCalled();
  });

  it('prevents state updates after unmount', async () => {
    let resolveFetch!: (val: any) => void;
    vi.mocked(getAggregatedSkills).mockReturnValueOnce(
      new Promise((res) => { resolveFetch = res; })
    );

    const { result, unmount } = renderHook(() => useAssistantSkills());
    
    unmount();
    resolveFetch([{ name: 'skill' }]);
    
    await new Promise(r => setTimeout(r, 10));
    expect(result.current.skills).toEqual([]);
  });
});
