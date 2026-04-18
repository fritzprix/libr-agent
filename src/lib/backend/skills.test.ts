import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  deleteUserSkill,
  getAggregatedSkills,
  getManagedSkillsOverview,
  importAssistantSkills,
  importUserSkills,
  installGitHubSkills,
  copyGlobalToAssistant,
  deleteAssistantSkill,
  previewGitHubSkillInstall,
  previewUserSkillImport,
  resetAssistantSkills,
  resetUserSkills,
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

  it('getManagedSkillsOverview calls safeInvoke without arguments', async () => {
    const mockResponse = {
      systemDirectory: '/system',
      userDirectory: '/user',
      systemSkills: [],
      userSkills: [],
      effectiveSkills: [],
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await getManagedSkillsOverview();

    expect(safeInvoke).toHaveBeenCalledWith('get_managed_skills_overview');
    expect(result).toEqual(mockResponse);
  });

  it('previewUserSkillImport calls safeInvoke with correct arguments', async () => {
    const mockResponse = { discoveredSkills: [], conflicts: [] };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await previewUserSkillImport('/path/to/skill.skill');

    expect(safeInvoke).toHaveBeenCalledWith('preview_user_skill_import', {
      filePath: '/path/to/skill.skill',
    });
    expect(result).toEqual(mockResponse);
  });

  it('importUserSkills calls safeInvoke with overwrite flag', async () => {
    const mockResponse = {
      importedNames: ['test-skill'],
      overwrittenNames: [],
      skippedNames: [],
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await importUserSkills('/path/to/skill.skill', true, ['skip-me']);

    expect(safeInvoke).toHaveBeenCalledWith('import_user_skills', {
      filePath: '/path/to/skill.skill',
      overwriteExisting: true,
      excludedSkillNames: ['skip-me'],
    });
    expect(result).toEqual(mockResponse);
  });

  it('previewGitHubSkillInstall calls safeInvoke with correct arguments', async () => {
    const mockResponse = { discoveredSkills: [], conflicts: [] };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await previewGitHubSkillInstall(
      'https://github.com/example/skills',
    );

    expect(safeInvoke).toHaveBeenCalledWith('preview_github_skill_install', {
      repoUrl: 'https://github.com/example/skills',
    });
    expect(result).toEqual(mockResponse);
  });

  it('installGitHubSkills calls safeInvoke with overwrite flag', async () => {
    const mockResponse = {
      importedNames: ['skill-a'],
      overwrittenNames: ['skill-b'],
      skippedNames: ['skill-c'],
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await installGitHubSkills(
      'https://github.com/example/skills',
      false,
      ['skip-me'],
    );

    expect(safeInvoke).toHaveBeenCalledWith('install_github_skills', {
      repoUrl: 'https://github.com/example/skills',
      overwriteExisting: false,
      excludedSkillNames: ['skip-me'],
    });
    expect(result).toEqual(mockResponse);
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

  it('deleteUserSkill calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await deleteUserSkill('test-skill');

    expect(safeInvoke).toHaveBeenCalledWith('delete_user_skill', {
      skillName: 'test-skill',
    });
    expect(result).toBe('success');
  });

  it('resetUserSkills calls safeInvoke without arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await resetUserSkills();

    expect(safeInvoke).toHaveBeenCalledWith('reset_user_skills');
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

  it('getSkillContent forwards optional scope arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('content');

    await getSkillContent('/path/to/skill', {
      assistantId: 'assistant-1',
      sessionId: 'session-1',
      workspacePath: '/workspace',
    });

    expect(safeInvoke).toHaveBeenCalledWith('get_skill_content', {
      skillPath: '/path/to/skill',
      assistantId: 'assistant-1',
      sessionId: 'session-1',
      workspacePath: '/workspace',
    });
  });
});
