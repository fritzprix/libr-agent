import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  getAggregatedSkills,
  importAssistantSkills,
  copyGlobalToAssistant,
  deleteAssistantSkill,
  resetAssistantSkills,
  getSkillContent,
} from './skills';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('skills backend wrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('getAggregatedSkills calls safeInvoke with correct arguments', async () => {
    const mockResponse = [{ name: 'test-skill', version: '1.0' }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await getAggregatedSkills('assistant-1');

    expect(safeInvoke).toHaveBeenCalledWith('get_aggregated_skills', {
      assistantId: 'assistant-1',
    });
    expect(result).toEqual(mockResponse);
  });

  it('importAssistantSkills calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await importAssistantSkills('assistant-1', '/path/to/skill.zip');

    expect(safeInvoke).toHaveBeenCalledWith('import_assistant_skills', {
      assistantId: 'assistant-1',
      filePath: '/path/to/skill.zip',
    });
    expect(result).toBe('success');
  });

  it('copyGlobalToAssistant calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await copyGlobalToAssistant('assistant-1', 'test-skill');

    expect(safeInvoke).toHaveBeenCalledWith('copy_global_to_assistant', {
      assistantId: 'assistant-1',
      skillName: 'test-skill',
    });
    expect(result).toBe('success');
  });

  it('deleteAssistantSkill calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await deleteAssistantSkill('assistant-1', 'test-skill');

    expect(safeInvoke).toHaveBeenCalledWith('delete_assistant_skill', {
      assistantId: 'assistant-1',
      skillName: 'test-skill',
    });
    expect(result).toBe('success');
  });

  it('resetAssistantSkills calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await resetAssistantSkills('assistant-1');

    expect(safeInvoke).toHaveBeenCalledWith('reset_assistant_skills', {
      assistantId: 'assistant-1',
    });
    expect(result).toBe('success');
  });

  it('getSkillContent calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('content');

    const result = await getSkillContent('/path/to/skill');

    expect(safeInvoke).toHaveBeenCalledWith('get_skill_content', {
      skillPath: '/path/to/skill',
    });
    expect(result).toBe('content');
  });
});
