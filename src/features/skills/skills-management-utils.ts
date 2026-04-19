import type { TFunction } from 'i18next';
import type {
  SkillsStatusMessage,
  SkillsVerificationStatus,
} from './skills-management-types';

export function formatImportSuccess(
  t: TFunction,
  importedCount: number,
  overwrittenCount: number,
  skippedCount: number,
): string {
  if (importedCount === 0 && skippedCount > 0) {
    return t('settings.skills.importSkippedOnly', {
      count: skippedCount,
      defaultValue_one: 'Skipped {{count}} conflicting skill',
      defaultValue_other: 'Skipped {{count}} conflicting skills',
    });
  }

  if (overwrittenCount > 0 && skippedCount > 0) {
    return t('settings.skills.importSuccessWithOverwriteAndSkipped', {
      count: importedCount,
      overwrittenCount,
      skippedCount,
      defaultValue_one:
        'Imported {{count}} skill ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
      defaultValue_other:
        'Imported {{count}} skills ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
    });
  }

  if (overwrittenCount > 0) {
    return t('settings.skills.importSuccessWithOverwrite', {
      count: importedCount,
      overwrittenCount,
      defaultValue_one:
        'Imported {{count}} skill ({{overwrittenCount}} overwritten)',
      defaultValue_other:
        'Imported {{count}} skills ({{overwrittenCount}} overwritten)',
    });
  }

  if (skippedCount > 0) {
    return t('settings.skills.importSuccessWithSkipped', {
      count: importedCount,
      skippedCount,
      defaultValue_one: 'Imported {{count}} skill ({{skippedCount}} skipped)',
      defaultValue_other:
        'Imported {{count}} skills ({{skippedCount}} skipped)',
    });
  }

  return t('settings.skills.importSuccess', {
    count: importedCount,
    defaultValue_one: 'Imported {{count}} skill',
    defaultValue_other: 'Imported {{count}} skills',
  });
}

export function getSkillsStatusMessage(
  verificationStatus: SkillsVerificationStatus,
  errorMessage: string,
  skillsCount: number,
  t: TFunction,
): SkillsStatusMessage {
  if (verificationStatus === 'loading') {
    return {
      kind: 'loading',
      text: t('settings.general.verifying', 'Verifying...'),
      tone: 'text-muted-foreground',
    };
  }

  if (verificationStatus === 'error') {
    return {
      kind: 'error',
      text:
        errorMessage ||
        t('settings.general.invalidDirectory', 'Invalid directory'),
      tone: 'text-destructive',
    };
  }

  return {
    kind: 'success',
    text: t('settings.skills.installedCount', {
      count: skillsCount,
      defaultValue: '{{count}} installed skills available',
    }),
    tone: 'text-success',
  };
}
