import type { SkillImportPreview, SkillMetadata } from '@/types/skills';

export interface PendingInstall {
  kind: 'local' | 'github';
  sourceValue: string;
  preview: SkillImportPreview;
  selectedOverwriteNames: string[];
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
