/**
 * Represents metadata for a skill.
 * This should match the Rust struct in `src-tauri/src/commands/skill_commands.rs`.
 */
export interface SkillMetadata {
  name: string;
  description: string;
  path: string;
  /**
   * Source of the skill.
   * - 'global': Located in the global skills directory.
   * - 'assistant': Located in the assistant's specific skills directory.
   * - 'workspace': Located in the workspace's local skills directory.
   * - undefined: Source is unknown or not relevant (e.g. raw directory scan).
   */
  source?: 'global' | 'assistant' | 'workspace';
  /**
   * Physical ownership of the skill content.
   * - 'system': Bundled read-only skill shipped with the app.
   * - 'user': Managed user-global skill installed into app data.
   * - 'assistant': Assistant-scoped override stored in app data.
   * - 'workspace': Workspace-local skill.
   */
  origin?: 'system' | 'user' | 'assistant' | 'workspace';
}

export interface ManagedSkillsOverview {
  systemDirectory: string;
  userDirectory: string;
  systemSkills: SkillMetadata[];
  userSkills: SkillMetadata[];
  effectiveSkills: SkillMetadata[];
}

export interface SkillImportCandidate {
  name: string;
  description: string;
}

export interface SkillImportConflict {
  name: string;
  existingOrigin: 'system' | 'user' | 'assistant' | 'workspace' | 'unknown';
  existingPath: string;
}

export interface SkillImportPreview {
  discoveredSkills: SkillImportCandidate[];
  conflicts: SkillImportConflict[];
}

export interface SkillImportResult {
  importedNames: string[];
  overwrittenNames: string[];
  skippedNames: string[];
}
