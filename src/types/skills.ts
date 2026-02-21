
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
   * - undefined: Source is unknown or not relevant (e.g. raw directory scan).
   */
  source?: 'global' | 'assistant' | string;
}
