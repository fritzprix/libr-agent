import type { SkillImportPreview, SkillMetadata } from '@/types/skills';

export interface PendingInstall {
  kind: 'local' | 'github';
  sourceValue: string;
  preview: SkillImportPreview;
  selectedOverwriteNames: string[];
  approveHooks: boolean;
  /** Answers to the plugin's `userConfig` options (#2926), keyed by manifest
   *  key. A sensitive value stays in this dialog's state only and goes to
   *  secure storage on the backend; it is never displayed again. */
  pluginOptions: Record<string, string>;
}

export type SkillsDirectoryScope = 'system' | 'user';

export type SkillsVerificationStatus = 'loading' | 'success' | 'error';

export interface SkillsStatusMessage {
  kind: SkillsVerificationStatus;
  text: string;
  tone: string;
}

export interface SkillsManagementDirectories {
  systemDirectory: string;
  userDirectory: string;
  systemSkills: SkillMetadata[];
  userSkills: SkillMetadata[];
}
