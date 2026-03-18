import { safeInvoke } from './core';
import type { SkillMetadata } from '@/types/skills';

interface SkillScopeOptions {
  sessionId?: string;
  workspacePath?: string;
}

/**
 * Fetches the aggregated skills for a given assistant, merging global and
 * assistant/workspace-specific skills.
 */
export async function getAggregatedSkills(
  assistantId?: string,
  options?: SkillScopeOptions,
): Promise<SkillMetadata[]> {
  const args: {
    assistantId?: string;
    sessionId?: string;
    workspacePath?: string;
  } = {};

  if (assistantId) {
    args.assistantId = assistantId;
  }
  if (options?.sessionId) {
    args.sessionId = options.sessionId;
  }
  if (options?.workspacePath) {
    args.workspacePath = options.workspacePath;
  }

  return safeInvoke<SkillMetadata[]>('get_aggregated_skills', args);
}

/**
 * Imports skills from a skill package file into an assistant's skill set.
 */
export async function importAssistantSkills(
  assistantId: string,
  filePath: string,
): Promise<string> {
  return safeInvoke<string>('import_assistant_skills', {
    assistantId,
    filePath,
  });
}

/**
 * Copies a global skill into the assistant's local skill set (override).
 */
export async function copyGlobalToAssistant(
  assistantId: string,
  skillName: string,
): Promise<string> {
  return safeInvoke<string>('copy_global_to_assistant', {
    assistantId,
    skillName,
  });
}

/**
 * Deletes an assistant-specific skill, reverting to the global version.
 */
export async function deleteAssistantSkill(
  assistantId: string,
  skillName: string,
): Promise<string> {
  return safeInvoke<string>('delete_assistant_skill', {
    assistantId,
    skillName,
  });
}

/**
 * Resets all assistant-specific skills, removing all overrides.
 */
export async function resetAssistantSkills(
  assistantId: string,
): Promise<string> {
  return safeInvoke<string>('reset_assistant_skills', { assistantId });
}

/**
 * Reads the full content of a skill's SKILL.md file.
 * `skillPath` is the absolute path as returned in `SkillMetadata.path`.
 */
export async function getSkillContent(
  skillPath: string,
  options?: SkillScopeOptions & { assistantId?: string },
): Promise<string> {
  const args: {
    skillPath: string;
    assistantId?: string;
    sessionId?: string;
    workspacePath?: string;
  } = { skillPath };

  if (options?.assistantId) {
    args.assistantId = options.assistantId;
  }
  if (options?.sessionId) {
    args.sessionId = options.sessionId;
  }
  if (options?.workspacePath) {
    args.workspacePath = options.workspacePath;
  }

  return safeInvoke<string>('get_skill_content', args);
}
